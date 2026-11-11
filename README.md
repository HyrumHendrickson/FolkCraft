# Folk Craft

[![CI](https://github.com/HyrumHendrickson/FolkCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/HyrumHendrickson/FolkCraft/actions/workflows/ci.yml)
[![CodeQL](https://github.com/HyrumHendrickson/FolkCraft/actions/workflows/codeql.yml/badge.svg)](https://github.com/HyrumHendrickson/FolkCraft/actions/workflows/codeql.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Folk Craft is a small voxel adventure game, in the spirit of Minecraft, with a folklore flavour. You explore a forest of
rivers, hills and clearings, hunt and cook, craft tools, and follow rainbows, ghostly trails and old stories to castles, a
mountain, caves under the world and islands in the sky.

**Folk Craft is a free, open source project.** It costs nothing to play, and anyone is welcome to read the code, change it,
learn from it and send improvements. (If you are an agent or a developer, start with [`AGENTS.md`](AGENTS.md).)

## Run it

You need [Rust](https://www.rust-lang.org/tools/install). Then, in this folder:

```sh
cargo run
```

On Linux you also need the ALSA development files (`libasound2-dev` on Debian and Ubuntu). Your worlds are saved in a `saves`
folder next to where you run the game. The first time, create a world from the menu; the **Help** button on the world list
explains everything in the game and is always up to date.

## Controls

| Key | What it does |
| --- | --- |
| W A S D, mouse | Walk and look |
| Space, Shift | Jump, sprint |
| Left mouse | Break a block, swing a weapon, shoot a bow (hold to break or swing) |
| Right mouse | Place, eat, cook, use what you hold, talk to ghosts, trade |
| 1 to 9, mouse wheel | Choose what you hold |
| E | Inventory and crafting (the Recipe Book button lists every recipe) |
| F2 | Settings (how far you can see) |
| Esc | Close a window, or save and return to the menu |

## Getting started

- Pick up sticks, grass and pebbles. Three grass make twine; twine and a stick make a bow drill, a bow, and with a stone a stone axe.
- Cut a tree with the axe, set a log down and light it with the bow drill. Cook meat on it, or smelt stones into metal on burning charcoal.
- Hunt deer, rabbits and quail with an axe or a bow. Eat to heal. Bears and moose hit back: keep your distance.
- Make a clay jar, fetch water, and cook a stew in a tin pot. A log makes five wooden bowls to carry the stew in.
- Keep a rabbit's foot in your hotbar and break pebbles for a Peep Stone. It shows a rainbow by day and a grand castle by night.

From there the world opens up: lakes with towers, a mountain with mine shafts and a Yeti, a jungle temple, the caves of the
Horizen, the Skylands (with a ghostly general store on the first island), and a last great hunt. Part of the fun is
finding out how. The in-game Help has the details when you want them.

## Playing together

On the world list, **Host Game** opens a world to your wifi network and **Join Game** finds games on it (or takes the host's address, which the host sees at the top right). Everyone shares the world, its creatures and bosses, but plays on their own: each player can travel to any dimension and use any item when they like. The host's computer runs the creatures and saves the world, and keeps each guest's pack for next time.

## Living with the game

- It is a work in progress: things change between versions, and older worlds still load.
- Found a bug, or want to add something? Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`AGENTS.md`](AGENTS.md), and run `cargo test` before sharing a change.

## Project

- [Contributing](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security policy](SECURITY.md): please report vulnerabilities privately
- [Changelog](CHANGELOG.md)

## License

Folk Craft is released under the [MIT License](LICENSE).
