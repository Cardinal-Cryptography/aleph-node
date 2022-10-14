use anyhow::Result;
use helpers::sign;

use self::contracts::AsContractInstance;
use crate::{
    test::button_game::helpers::{assert_recv, setup_button_test, ButtonTestContext},
    Config,
};

mod contracts;
mod helpers;

pub fn early_bird_special(config: &Config) -> Result<()> {
    let ButtonTestContext {
        conn,
        button,
        mut events,
        authority,
        marketplace,
        ..
    } = setup_button_test(config)?;

    let deadline_old = button.deadline(&conn)?;
    button.reset(&sign(&conn, authority.clone()))?;

    assert_recv(
        &mut events,
        |event| {
            event.contract == *button.as_contract().address()
                && event.ident == Some("GameReset".to_string())
        },
        "GameReset event",
    );
    assert_recv(
        &mut events,
        |event| {
            event.contract == *marketplace.as_contract().address()
                && event.ident == Some("Reset".to_string())
        },
        "Marketplace Reset event",
    );
    let deadline_new = button.deadline(&conn)?;
    assert!(deadline_new > deadline_old, "Deadline update");

    Ok(())
}
