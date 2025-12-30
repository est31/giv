# giv

Pure Rust `gitk` clone for the terminal

The tool builds on [gitoxide](https://github.com/GitoxideLabs/gitoxide) as well as [ratatui](https://github.com/ratatui/ratatui).

## Motivation

### gitk is the best

`gitk` is one of the first guis for git and in my personal opinion, it is still better than most
of the guis that came after it. It shows the most relevant content right away, without hiding it somewhere.

The usual goal for a git ui is to make changing state easier (checking out a branch, making a commit,
merging a branch, etc), so the focus is not mainly on showing the state.

Thus, most if not all git uis fail the simple test: is there a way, with a single click or
a single keypress, to switch between the diff view of different commits?

`gitui` for example takes 6 keypresses.

One can emulate something like this via `git log -p`, by using the less search `/^commit` then `n`/`N`,
but it's not perfect and takes a lot of time to "set up".

There is more tests for the usability of a git tool:
- is the diff shown "unified" i.e. you only need to scroll down and see it for all files, or is there
  interactivity needed to switch between files?
- is the diff shown in the same window as the list of commits, or do you need interactivity to switch
  from seeing the commit list and the diff list?

The more complicated this is, and the more clicks are needed, the harder it is to get an overview of
changes that happen in a certain commit

### why write my own?

Many people still use gitk, and many people wrote their own terminal clone of gitk for similar
motivations. Why did I write my own instead of use one of the existing alternatives?

First, it's [fun](https://jyn.dev/i-m-just-having-fun/) to do so, second, I wanted to
try out both gitoxide and ratatui for a non-trivial project.

If I write my own tool, I don't need to convince the maintainers to merge my changes, I can
scratch my itch in precisely the way I want it to be scratched.

## Alternative projects

- [gitv](https://github.com/gregsexton/gitv) - gitk for vim (unmaintained)
- [gitj](https://github.com/chjj/gitj) gitk in your terminal (unmaintained)
- [gitt](https://github.com/medwards/gitt) - Git repository viewer for your terminal
- [gitui](https://github.com/gitui-org/gitui) - Blazing 💥 fast terminal-ui for git

## MSRV policy

We depend on a bunch of components, but we try to support rust versions at least 6 releases back.

### License
[license]: #license

This tool is distributed under the terms of both the MIT license
and the Apache License (Version 2.0), at your option.

See [LICENSE](LICENSE) for details.

#### License of your contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.