use std::{thread, time::Duration};

use aleph_client::{contract_transcode::Value, AccountId};
use anyhow::Result;
use assert2::{assert, let_assert};
use helpers::sign;
use log::info;
use sp_core::Pair;

use crate::{
    test::button_game::helpers::{
        assert_recv, assert_recv_id, mega, refute_recv_id, setup_button_test, setup_dex_test,
        wait_for_death, ButtonTestContext, DexTestContext,
    },
    Config,
};

mod contracts;
mod helpers;

/// Test trading on simple_dex.
///
/// The scenario does the following (given 3 tokens A, B, C):
///
/// 1. Enables A <-> B, and A -> C swaps.
/// 2. Adds (A, 2000M), (B, 5000M), (C, 10000M) of liquidity.
/// 3. Makes a swap A -> B and then B -> A for the amount of B received in the first swap.
/// 4. Makes a swap A -> B expecting negative slippage (this should fail).
/// 5. Checks that the price after the two swaps is the same as before (with a dust allowance of 1 for rounding).
/// 6. Checks that it's possible to make an A -> C swap, but impossible to make a C -> A swap.
pub fn simple_dex(config: &Config) -> Result<()> {
    let DexTestContext {
        conn,
        authority,
        account,
        dex,
        token1,
        token2,
        token3,
        mut events,
    } = setup_dex_test(config)?;
    let authority_conn = &sign(&conn, &authority);
    let account_conn = &sign(&conn, &account);
    let token1 = token1.as_ref();
    let token2 = token2.as_ref();
    let token3 = token3.as_ref();
    let dex = dex.as_ref();

    dex.add_swap_pair(authority_conn, token1.into(), token2.into())?;
    assert_recv_id(&mut events, "SwapPairAdded");

    dex.add_swap_pair(authority_conn, token2.into(), token1.into())?;
    assert_recv_id(&mut events, "SwapPairAdded");

    dex.add_swap_pair(authority_conn, token1.into(), token3.into())?;
    assert_recv_id(&mut events, "SwapPairAdded");

    token1.mint(authority_conn, &authority.public().into(), mega(3000))?;
    token2.mint(authority_conn, &authority.public().into(), mega(5000))?;
    token3.mint(authority_conn, &authority.public().into(), mega(10000))?;

    token1.approve(authority_conn, &dex.into(), mega(3000))?;
    token2.approve(authority_conn, &dex.into(), mega(5000))?;
    token3.approve(authority_conn, &dex.into(), mega(10000))?;
    dex.deposit(
        authority_conn,
        &[
            (token1, mega(3000)),
            (token2, mega(5000)),
            (token3, mega(10000)),
        ],
    )?;

    assert!(
        dex.out_given_in(account_conn, token1, token2, 100).is_ok(),
        "out_given_in should always return"
    );

    let more_than_liquidity = mega(1_000_000);
    assert!(dex
        .swap(account_conn, token1, 100, token2, more_than_liquidity)
        .is_err());

    let initial_amount = mega(100);
    token1.mint(authority_conn, &account.public().into(), initial_amount)?;
    let expected_output = dex.out_given_in(account_conn, token1, token2, initial_amount)?;
    assert!(expected_output > 0);

    let at_most_10_percent_slippage = expected_output * 9 / 10;
    token1.approve(account_conn, &dex.into(), initial_amount)?;
    dex.swap(
        account_conn,
        token1,
        initial_amount,
        token2,
        at_most_10_percent_slippage,
    )?;
    assert_recv_id(&mut events, "Swapped");
    assert!(token2.balance_of(&conn, &account.public().into())? == expected_output);

    token2.approve(account_conn, &dex.into(), expected_output)?;
    dex.swap(account_conn, token2, expected_output, token1, mega(90))?;
    assert_recv_id(&mut events, "Swapped");

    let balance_after = token1.balance_of(&conn, &account.public().into())?;
    assert!(initial_amount.abs_diff(balance_after) <= 1);
    assert!(
        dex.out_given_in(account_conn, token1, token2, initial_amount)?
            .abs_diff(expected_output)
            <= 1
    );

    token1.approve(account_conn, &dex.into(), balance_after)?;
    let unreasonable_slippage = expected_output * 11 / 10;
    dex.swap(
        account_conn,
        token1,
        balance_after,
        token2,
        unreasonable_slippage,
    )?;
    refute_recv_id(&mut events, "Swapped");

    dex.swap(account_conn, token1, balance_after, token3, mega(90))?;
    assert_recv_id(&mut events, "Swapped");
    let balance_token3 = token3.balance_of(&conn, &account.public().into())?;
    token3.approve(account_conn, &dex.into(), balance_token3)?;
    dex.swap(account_conn, token3, balance_token3, token1, mega(90))?;
    refute_recv_id(&mut events, "Swapped");

    Ok(())
}

/// Tests trading on the marketplace.
///
/// The scenario:
///
/// 1. Buys a ticket without setting the max price (this should succeed).
/// 2. Tries to buy a ticket with setting the max price too low (this should fail).
/// 3. Tries to buy a ticket with setting the max price appropriately (this should succeed).
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
