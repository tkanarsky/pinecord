# Pinecord

Discord Rich Presence integration for the Pinecil V2 soldering iron. Shows live temperature, voltage, andpower usage in your discord status, because why the hell not

## Use cases

- Criticizing your friends' soldering skills
- Showing off your sick 28v EPR power supply
- Making sure said friends aren't dozing off with a running soldering iron

## Requirements

- Pinecil V2 iron
  - Make sure Bluetooth is enabled under advanced settings
- Computer with Bluetooth 4.0 capability (literally anything newer than 2012)
- Discord desktop client
- Linux (tested under Ubuntu 22.04), maybe Mac and Windows (lmk)

## Usage
1. Clone this repo
2. `cargo install -- path .`
3. Run `pinecord`
4. Discord will automatically update your status
5. You can leave it running in the background. When you unplug the pinecil the status disappears
6. To exit, press Ctrl+C

## License

MIT

## Author

tkanarsky (tkanarsky@outlook.com)