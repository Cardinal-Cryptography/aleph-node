use std::{thread, time::Duration};

use aleph_client::contract_transcode::Value;
use anyhow::Result;
use assert2::{assert, let_assert};
use helpers::sign;
use log::info;

use crate::{
    test::button_game::{
        contracts::ToAccount,
        helpers::{
            assert_recv, assert_recv_id, refute_recv_id, setup_button_test, wait_for_death,
            ButtonTestContext,
        },
    },
    Config,
};

mod contracts;
mod helpers;

pub fn marketplace(config: &Config) -> Result<()> {
    let ButtonTestContext {
        conn,
        authority,
        player,
        marketplace,
        ticket_token,
        reward_token,
        mut events,
        ..
    } = setup_button_test(config)?;

    marketplace.reset(&sign(&conn, authority.clone()))?;
    assert_recv_id(&mut events, "Reset");
    ticket_token.transfer(
        &sign(&conn, authority.clone()),
        &marketplace.to_account(),
        2,
    )?;

    let early_price = marketplace.price(&conn)?;
    thread::sleep(Duration::from_secs(2));
    let later_price = marketplace.price(&conn)?;
    assert!(later_price < early_price);

    let player_balance = 100 * later_price;
    reward_token.mint(
        &sign(&conn, authority.clone()),
        &player.to_account(),
        player_balance,
    )?;
    reward_token.approve(
        &sign(&conn, player.clone()),
        &marketplace.to_account(),
        later_price,
    )?;
    marketplace.buy(&sign(&conn, player.clone()), None)?;

    let event = assert_recv_id(&mut events, "Bought");
    assert!(event.contract == marketplace.to_account());
    let_assert!(Some(&Value::UInt(price)) = event.data.get("price"));
    assert!(price <= later_price);
    let_assert!(Some(Value::Literal(account_id)) = event.data.get("account_id"));
    assert!(account_id == &player.to_account().to_string());
    assert!(ticket_token.balance_of(&conn, &player.to_account())? == 1);
    assert!(reward_token.balance_of(&conn, &player.to_account())? <= player_balance - price);
    assert!(marketplace.price(&conn)? > price);

    let latest_price = marketplace.price(&conn)?;

    info!("Setting max price too low");
    marketplace.buy(&sign(&conn, player.clone()), Some(latest_price / 2))?;
    refute_recv_id(&mut events, "Bought");
    assert!(ticket_token.balance_of(&conn, &player.to_account())? == 1);

    info!("Setting max price high enough");
    marketplace.buy(&sign(&conn, player.clone()), Some(latest_price * 2))?;
    assert_recv_id(&mut events, "Bought");
    assert!(ticket_token.balance_of(&conn, &player.to_account())? == 2);

    Ok(())
}

pub fn early_bird_special_reset(config: &Config) -> Result<()> {
    let ButtonTestContext {
        conn,
        button,
        mut events,
        authority,
        marketplace,
        ticket_token,
        ..
    } = setup_button_test(config)?;

    let deadline_old = button.deadline(&conn)?;
    let marketplace_initial = ticket_token.balance_of(&conn, &marketplace.to_account())?;
    ticket_token.transfer(&sign(&conn, authority.clone()), &button.to_account(), 1)?;

    wait_for_death(&conn, &button)?;
    button.reset(&sign(&conn, authority.clone()))?;

    let _ = assert_recv(
        &mut events,
        |event| {
            event.contract == button.to_account() && event.ident == Some("GameReset".to_string())
        },
        "GameReset event",
    );
    let _ = assert_recv(
        &mut events,
        |event| {
            event.contract == marketplace.to_account() && event.ident == Some("Reset".to_string())
        },
        "Marketplace Reset event",
    );
    let deadline_new = button.deadline(&conn)?;
    assert!(deadline_new > deadline_old);
    assert!(ticket_token.balance_of(&conn, &marketplace.to_account())? == marketplace_initial + 1);

    Ok(())
}

pub fn early_bird_special_play(config: &Config) -> Result<()> {
    let ButtonTestContext {
        conn,
        button,
        mut events,
        authority,
        ticket_token,
        reward_token,
        player,
        ..
    } = setup_button_test(config)?;

    ticket_token.transfer(&sign(&conn, authority.clone()), &player.to_account(), 2)?;
    wait_for_death(&conn, &button)?;
    button.reset(&sign(&conn, authority.clone()))?;
    let old_button_balance = ticket_token.balance_of(&conn, &button.to_account())?;

    ticket_token.approve(&sign(&conn, player.clone()), &button.to_account(), 2)?;
    button.press(&sign(&conn, player.clone()))?;

    let event = assert_recv_id(&mut events, "ButtonPressed");
    let_assert!(Some(&Value::UInt(early_presser_score)) = event.data.get("score"));
    assert!(event.data.get("by") == Some(&Value::Literal(player.to_account().to_string())));
    assert!(reward_token.balance_of(&conn, &player.to_account())? == early_presser_score);
    assert!(early_presser_score > 0);
    assert!(ticket_token.balance_of(&conn, &player.to_account())? == 1);
    assert!(ticket_token.balance_of(&conn, &button.to_account())? == old_button_balance + 1);

    info!("Waiting before pressing again");
    thread::sleep(Duration::from_secs(5));

    button.press(&sign(&conn, player.clone()))?;
    let event = assert_recv_id(&mut events, "ButtonPressed");
    let_assert!(Some(&Value::UInt(late_presser_score)) = event.data.get("score"));
    assert!(early_presser_score > late_presser_score);
    let total_score = early_presser_score + late_presser_score;
    assert!(reward_token.balance_of(&conn, &player.to_account())? == total_score);

    wait_for_death(&conn, &button)?;
    button.reset(&sign(&conn, authority.clone()))?;
    assert_recv_id(&mut events, "Reset");

    let pressiah_score = total_score / 4;
    assert!(reward_token.balance_of(&conn, &player.to_account())? == total_score + pressiah_score);

    Ok(())
}
