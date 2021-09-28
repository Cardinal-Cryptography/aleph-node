use clap::{App, Arg, ArgMatches};
use sp_core::{sr25519, Pair};
use std::env;
use std::fs;

#[derive(Clone)]
pub struct Config {
    pub url: String,
    pub sudo: sr25519::Pair,
    pub accounts: Vec<sr25519::Pair>,
}

pub fn build_app() -> App<'static> {
    App::new("e2e-test-client")
        .version("0.1.0")
        .about("tool for e2e testing of blockchains powered by the AlephBFT finality gadget")
        .arg(
            Arg::new("host")
                .short('h')
                .long("host")
                .takes_value(true)
                .default_value("127.0.0.1")
                .about("host to connect to"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .takes_value(true)
                .default_value("9943")
                .about("websocket RPC port to connect to"),
        )
        .arg(
            Arg::new("base-path")
                .short('b')
                .long("base-path")
                .takes_value(true)
                .required(true)
                .about("root account secret phrase"),
        )
        .arg(
            Arg::new("account-key-file")
                .short('k')
                .long("account-key-file")
                .takes_value(true)
                .default_value("account_secret")
                .about("root account secret phrase"),
        )
        .arg(
            Arg::new("account-ids")
                .short('i')
                .long("account-ids")
                .multiple_values(true)
                .takes_value(true)
                .required(true)
                .about("account ids of the nodes in the committee"),
        )
        .arg(
            Arg::new("sudo-account-id")
                .short('s')
                .long("sudo-account-id")
                .takes_value(true)
                .about(
                    "account id of the root. Defaults to the first account id if the arguments is not passed explicitely",
                ),
        )
}

pub fn build_config(matches: &ArgMatches) -> Config {
    // NOTE : could be parsed as BasePath from OS string
    let base_path = matches.value_of("base-path").unwrap();
    let key_file = matches.value_of("account-key-file").unwrap();

    Config {
        url: format!(
            "ws://{}:{}",
            matches.value_of("host").unwrap(),
            matches.value_of("port").unwrap()
        ),
        accounts: matches
            .values_of("account-ids")
            .unwrap()
            .map(|id| {
                let file = format!("{}/{}/{}", &base_path, id, key_file);
                read_keypair(file)
            })
            .collect::<Vec<sr25519::Pair>>(),
        sudo: read_keypair(format!(
            "{}/{}/{}",
            &base_path,
            matches.value_of("sudo-account-id").unwrap(),
            key_file
        )),
    }
}

fn read_keypair(file: String) -> sr25519::Pair {
    let phrase = fs::read_to_string(&file)
        .unwrap_or_else(|_err| panic!("Could not read the phrase form the secret file: {}", file));
    sr25519::Pair::from_phrase(&phrase, None)
        .expect("not a secret phrase")
        .0
}

pub fn get_env_var(var: &str, default: Option<String>) -> String {
    match env::var(var) {
        Ok(v) => v,
        Err(_) => match default {
            None => panic!("Missing ENV variable: {} not defined in environment", var),
            Some(d) => d,
        },
    }
}
