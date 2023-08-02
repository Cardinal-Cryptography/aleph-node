# Mainnet

`5Cck6AprxZGWGYDAqMLLj83AA2vmaDaUf3LJHnL2NXcwqpUv` is the mainnet authority for the dapp contracts.
Make sure you have the seed phrase stored in an `MAINNET_THE_BUTTON_AUTHORITY_SEED` ENV var before proceeding.

## Deployment

Export the constants with setting and run the deployment script:

```bash
source ./contracts/env/mainnet && ./contracts/scripts/deploy.sh
```

This creates `adresses.json` file that you should be renamed to `addresses.mainnet.json`
Now to seed the `Marketplace` and `DEX` contracts with some initial state:

```bash
source ./contracts/env/mainnet && ./contracts/scripts/seed.sh
```

### Airdrop

Final step is to airdrop the ticket tokens to the desiganted players accounts:

```bash
source ./contracts/env/mainnet && ./contracts/scripts/airdrop.sh
```
## Maintain

There will have to be some ongoing light maintainance of the games.
Firstly we should periodically top up the DEX with some wrapped Azero liquidity, there is the `add_liquidity` function that can be called to wrap some of the native token and tranfer it to the DEX.
As a rule of thumb send 1K tokens with every round.

Secondly after game's end to reward ThePressiah's and begin next round of the games use `reset_game` function.
If after some round should we decide not to reset it further ThePressiah of the last round should still be rewarded and we have the `reward_pressiah` function for that.

### Periodic maintainance with CRON

For periodic maintainance add the script to the crontab:

```bash
sudo crontab -e
```

Add add the job:

```
@daily source ./contracts/env/mainnet && ./contracts/scripts/maintain.sh
```

To check the cron logs:

```bash
sudo journalctl --since yesterday -u cron.service
```
