# Connect Four for Surena

This is an implementation of the _Connect Four_ game for the
[_surena_](https://github.com/RememberOfLife/surena/) game engine.

## Building

1. Test the project:
   ```
   $ cargo test
   ```
2. Create a release build:
   ```
   $ cargo build --release
   ```
3. Locate the plugin at `./target/release/libsurena_connect_four.so`

## Running

```
$ surena --game-plugin ./libsurena_connect_four.so
```

## Options

Imports options in the following format: `7x6@4`.

The option string consists of three separated numbers with the meaning of
column count, row count, and minimum number of connected pieces for winning.

## State

Imports state in the following format: `XOOXXXO/XOOX//OXXO#x`.

Each sequence of `X`s and `O`s between `/`s represents a column of stones from
bottom to top.
A hashtag-separated, lower-case letter at the end indicates who plays next.
An upper-case letter indicates that this player has won.
A dash indicates a draw.

## TODOs

- Create a frontend
- Implement more optional API methods

## Libraries

This project uses the following libraries:

- [_surena_game_rs_](https://github.com/vilaureu/surena_game_rs) under the
  [_MIT License_](https://github.com/vilaureu/surena_game_rs/blob/main/LICENSE)
- [_surena_](https://github.com/RememberOfLife/surena/) under the
  [_MIT License_](https://github.com/RememberOfLife/surena/blob/master/LICENSE)

## License

See the `LICENSE` file.
