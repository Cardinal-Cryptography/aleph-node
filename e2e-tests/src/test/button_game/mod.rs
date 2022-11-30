use std::{thread, time::Duration};

use aleph_client::{contract_transcode::Value, AccountId};
use anyhow::Result;
use assert2::{assert, let_assert};
use helpers::{sign, update_marketplace_metadata_to_v2};
use log::info;

use crate::{
    test::button_game::{
        helpers::{
            assert_recv, assert_recv_id, refute_recv_id, setup_button_test, wait_for_death,
            ButtonTestContext,
        },
    },
    Config,
};

mod contracts;
mod helpers;

/// Tests trading on the marketplace (with update).
///
/// The scenario:
///
/// 1. Buys a ticket without setting the max price (this should succeed).
/// 2. Tries to buy a ticket with setting the max price too low (this should fail).
/// 3. Tries to buy a ticket with setting the max price appropriately (this should succeed).
///
/// Additionally there is an update performed amidst contract's operation.
pub fn marketplace_with_update(config: &Config) -> Result<()> {
    let ButtonTestContext {
        conn,
        authority,
        player,
        mut marketplace,
        ticket_token,
        reward_token,
        mut events,
        ..
    } = setup_button_test(config, &config.test_case_params.early_bird_special)?;
    let player = &player;

    marketplace.reset(&sign(&conn, &authority))?;
    assert_recv_id(&mut events, "Reset");
    ticket_token.transfer(&sign(&conn, &authority), &marketplace.as_ref().into(), 2)?;

    let early_price = marketplace.price(&conn)?;
    thread::sleep(Duration::from_secs(2));
    let later_price = marketplace.price(&conn)?;
    assert!(later_price < early_price);

    let player_balance = 100 * later_price;
    reward_token.mint(&sign(&conn, &authority), &player.into(), player_balance)?;
    reward_token.approve(
        &sign(&conn, player),
        &marketplace.as_ref().into(),
        later_price,
    )?;
    marketplace.buy(&sign(&conn, player), None)?;

    // Upgrade begins

    // Performing code upgrade
    let set_code_result = marketplace.set_code(
        &sign(&conn, &authority),
        config
            .test_case_params
            .marketplace_v2_code_hash
            .as_ref()
            .expect("New code's code_hash must be specified."),
        None, // Alternative for performing "atomic" upgrade + migration (make sure that selector to migrate method is correct)
              // Some("0x060d3f50".to_string())
    );
    info!("Trying to set code actual hash_code: {:?}", set_code_result);
    assert!(set_code_result.is_ok());

    // Change the metadata (keeping the old address)
    marketplace = update_marketplace_metadata_to_v2(marketplace, &config);

    let migration_result = marketplace.migrate(&sign(&conn, &authority));
    info!(
        "Trying to perform migration after changing the metadata: {:?}",
        migration_result
    );
    assert!(migration_result.is_ok());

    // Check if migration was actually performed
    //
    // Will fail when:
    // - Upgrade was not successful/not performed, or
    // - Migration was not successful/not performed
    assert!(matches!(
        marketplace.migration_performed(&sign(&conn, &authority)),
        Ok(true)
    ));

    // Upgrade ends

    let event = assert_recv_id(&mut events, "Bought");
    let player_account: AccountId = player.into();
    assert!(event.contract == marketplace.as_ref().into());
    let_assert!(Some(&Value::UInt(price)) = event.data.get("price"));
    assert!(price <= later_price);
    let_assert!(Some(Value::Literal(acc_id)) = event.data.get("account_id"));
    assert!(acc_id == &player_account.to_string());
    assert!(ticket_token.balance_of(&conn, &player.into())? == 1);
    assert!(reward_token.balance_of(&conn, &player.into())? <= player_balance - price);
    assert!(marketplace.price(&conn)? > price);

    let latest_price = marketplace.price(&conn)?;

    info!("Setting max price too low");
    marketplace.buy(&sign(&conn, player), Some(latest_price / 2))?;
    refute_recv_id(&mut events, "Bought");
    assert!(ticket_token.balance_of(&conn, &player.into())? == 1);

    info!("Setting max price high enough");
    marketplace.buy(&sign(&conn, player), Some(latest_price * 2))?;
    assert_recv_id(&mut events, "Bought");
    assert!(ticket_token.balance_of(&conn, &player.into())? == 2);

    Ok(())
}

/// Tests resetting the button game.
pub fn button_game_reset(config: &Config) -> Result<()> {
    let ButtonTestContext {
        conn,
        button,
        mut events,
        authority,
        marketplace,
        ticket_token,
        ..
    } = setup_button_test(config, &config.test_case_params.early_bird_special)?;

    let deadline_old = button.deadline(&conn)?;
    let marketplace_initial = ticket_token.balance_of(&conn, &marketplace.as_ref().into())?;
    ticket_token.transfer(&sign(&conn, &authority), &button.as_ref().into(), 1)?;

    wait_for_death(&conn, &button)?;
    button.reset(&sign(&conn, &authority))?;

    let _ = assert_recv(
        &mut events,
        |event| {
            event.contract == button.as_ref().into() && event.ident == Some("GameReset".to_string())
        },
        "GameReset event",
    );
    let _ = assert_recv(
        &mut events,
        |event| {
            event.contract == marketplace.as_ref().into()
                && event.ident == Some("Reset".to_string())
        },
        "Marketplace Reset event",
    );
    let deadline_new = button.deadline(&conn)?;
    assert!(deadline_new > deadline_old);
    assert!(
        ticket_token.balance_of(&conn, &marketplace.as_ref().into())? == marketplace_initial + 1
    );

    Ok(())
}

pub fn early_bird_special(config: &Config) -> Result<()> {
    button_game_play(
        config,
        &config.test_case_params.early_bird_special,
        |early_presser_score, late_presser_score| {
            assert!(early_presser_score > late_presser_score);
        },
    )
}

pub fn back_to_the_future(config: &Config) -> Result<()> {
    button_game_play(
        config,
        &config.test_case_params.back_to_the_future,
        |early_presser_score, late_presser_score| {
            assert!(early_presser_score < late_presser_score);
        },
    )
}

pub fn the_pressiah_cometh(config: &Config) -> Result<()> {
    button_game_play(
        config,
        &config.test_case_params.the_pressiah_cometh,
        |early_presser_score, late_presser_score| {
            assert!(early_presser_score == 1);
            assert!(late_presser_score == 2);
        },
    )
}

/// Tests a basic scenario of playing the game.
///
/// The scenario:
///
/// 1. Resets the button.
/// 2. Gives 2 tickets to the player.
/// 3. Presses the button.
/// 4. Waits a bit and presses the button again.
/// 5. Waits until the button dies and checks the pressiah's score.
///
/// Passes the scores received by an early presser and late presser to `score_check` so that different scoring rules
/// can be tested generically.
fn button_game_play<F: Fn(u128, u128)>(
    config: &Config,
    button_contract_address: &Option<String>,
    score_check: F,
) -> Result<()> {
    let ButtonTestContext {
        conn,
        button,
        mut events,
        authority,
        ticket_token,
        reward_token,
        player,
        ..
    } = setup_button_test(config, button_contract_address)?;
    let player = &player;

    ticket_token.transfer(&sign(&conn, &authority), &player.into(), 2)?;
    wait_for_death(&conn, &button)?;
    button.reset(&sign(&conn, &authority))?;
    let old_button_balance = ticket_token.balance_of(&conn, &button.as_ref().into())?;

    ticket_token.approve(&sign(&conn, player), &button.as_ref().into(), 2)?;
    button.press(&sign(&conn, player))?;

    let event = assert_recv_id(&mut events, "ButtonPressed");
    let player_account: AccountId = player.into();
    let_assert!(Some(&Value::UInt(early_presser_score)) = event.data.get("score"));
    assert!(event.data.get("by") == Some(&Value::Literal(player_account.to_string())));
    assert!(reward_token.balance_of(&conn, &player.into())? == early_presser_score);
    assert!(early_presser_score > 0);
    assert!(ticket_token.balance_of(&conn, &player.into())? == 1);
    assert!(ticket_token.balance_of(&conn, &button.as_ref().into())? == old_button_balance + 1);

    info!("Waiting before pressing again");
    thread::sleep(Duration::from_secs(5));

    button.press(&sign(&conn, player))?;
    let event = assert_recv_id(&mut events, "ButtonPressed");
    let_assert!(Some(&Value::UInt(late_presser_score)) = event.data.get("score"));
    score_check(early_presser_score, late_presser_score);
    let total_score = early_presser_score + late_presser_score;
    assert!(reward_token.balance_of(&conn, &player.into())? == total_score);

    wait_for_death(&conn, &button)?;
    button.reset(&sign(&conn, &authority))?;
    assert_recv_id(&mut events, "Reset");

    let pressiah_score = total_score / 4;
    assert!(reward_token.balance_of(&conn, &player.into())? == total_score + pressiah_score);

    Ok(())
}
