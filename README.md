# Connect Four for Mirabel/Surena

This is an implementation of the _Connect Four_ game for the
[_surena_](https://github.com/RememberOfLife/surena) game engine and the
[_mirabel_](https://github.com/RememberOfLife/mirabel) game GUI.

## Building

1. Test the project:
   ```
   $ cargo test
   ```
2. Create a release build:
   ```
   $ cargo build --release
   ```
3. Locate the plugin at `./target/release/libmirabel_connect_four.so`

## Running

Using _surena_:
```
$ surena --game-plugin ./libmirabel_connect_four.so
```

Or by loading the plugin into _mirabel_ using the plugin manager.

## Options for the Game Plugin

Imports options in the following format: `7x6@4`.

The option string consists of three separated numbers with the meaning of
column count, row count, and minimum number of connected pieces for winning.

## State Format Used by the Game Plugin

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
- [_mirabel_rs_](https://github.com/vilaureu/mirabel_rs) under the
  [_MIT License_](https://github.com/vilaureu/mirabel_rs/blob/main/LICENSE)

## License

See the `LICENSE` file.
