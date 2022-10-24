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
            assert_recv, assert_recv_id, setup_button_test, wait_for_death, ButtonTestContext,
        },
    },
    Config,
};

mod contracts;
mod helpers;

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
