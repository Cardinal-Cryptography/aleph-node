# TheButton

Series of smart contract based games, with varying tokenomics.
TheButton is based on the famous [game](https://en.wikipedia.org/wiki/The_Button_(Reddit)) on reddit.
- Button lives for a set time.
- Pressing the button extends it's life.
- Users are rewarded for playing the game.
- Everybody can play only once.

```
  |______|_______|
  |      |       |
start   now    deadline
```

## Red Button

There is a pre-minted amount of red tokens (a classic ERC20).
Users are rewarded for clicking as early on as possible, maximizing TheButtons life.

```
score = deadline - now
```

There are two built-in incentives:
* playing for score: If you clicked in 10'th second of TheButtons life set for example to 900 blocks you get rewarded based on score of 900-10=890 (and the button's life now will end at block 910).
* playing to be ThePressiah: the last player to click get's 50% of the total rewards pool.


## Yellow button

Similar to button red, but in that scenario the awards are reversed - players get rewarded for extending the button's life further into the future, i.e.:

```
score = now - start
```

## Blue button

Game continues in perpetuity (but in practice as long as there are accounts that can still play it)
- In each iteration of the game TheButton lives for a number of blocks
- Clicking TheButton resets it's countdown timer (extending the button's life further into the future)
- Blue Tokens are continuously minted at the end of each iteration
- Players are rewarded for playing, with the ultimate goal of being the Pressiah (the last person to click the button)
- Reward rules:
  - If you’re not ThePressiah, you get *k* tokens if you pressed the button as *k-th* person in a row.
  - ThePressiah of the iteration with *k* button presses gets [k*(k+1)]/2 tokens.

## Deployment

```bash
yarn deps
yarn watch
source env/dev
node scripts/deploy.js
M-x cider-connect-clojurescript
```
