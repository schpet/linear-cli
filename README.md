# linear cli

linear's UI is incredibly good but it can't take advantage of me already being
in my project, on the command line. e.g.

- linear makes me switch context from where i'm working to their app or website
- linear doesn't know what team i'm currently acting on, so i have to navigate
  to it
- linear can suggest a git branch, but it makes me do the work of creating or
  switching to that branch
- linear's suggested git branch doesn't account for it already existing and
  having a merged pull request

this cli solves this: it knows what you're working on, does the work of managing
branches, and will write your pull request details for you.

see [the full list of commands](#commands) below.

## install

### homebrew

```
brew install schpet/tap/linear
```

### deno via jsr

```bash
deno install --allow-env --allow-sys --allow-run --allow-read --allow-net -g -n linear jsr:@schpet/linear-cli
```

### binaries

https://github.com/schpet/linear-cli/releases/latest

### local dev

```bash
git clone https://github.com/schpet/linear-cli
cd linear-cli
deno task install
```

## setup

1. create an API key at https://linear.app/settings/account/security (requires member access,
   not available for guest accounts)

2. add the API key to your shell environment:

   ```sh
   # in ~/.bashrc or ~/.zshrc:
   export LINEAR_API_KEY="lin_api_..."

   # or in fish:
   set -Ux LINEAR_API_KEY "lin_api_..."
   ```

3. run the configuration wizard:

   ```sh
   linear config
   ```

   This will create a `.linear.toml` config file in your repository with your
   workspace and team settings.

the CLI works best when your git branches include Linear issue IDs (e.g.
`eng-123-my-feature`). use `linear issue start` or linear UI's 'copy git branch
name' button and
[related automations](https://linear.app/docs/account-preferences#git-related-automations).

## commands

### issue commands

the current issue is determined by the issue id in the current git branch name.
note that
[Linear's GitHub integration](https://linear.app/docs/github#branch-format) will
suggest these branch names.

```bash
linear issue view      # view issue details in terminal
linear issue view -w   # open issue in web browser
linear issue view -a   # open issue in Linear.app
linear issue id        # prints the issue id from current branch (e.g., "ENG-123")
linear issue title     # prints just the issue title
linear issue url       # prints the Linear.app URL for the issue
linear issue pr        # creates a GitHub PR with issue details via `gh pr create`
linear issue list      # list your issues in a table view (supports -s/--state and --sort)
linear issue list -w   # open issue list in web browser
linear issue list -a   # open issue list in Linear.app
linear issue start    # create/switch to issue branch and mark as started
```

### other commands

```bash
linear --help          # show all commands
linear --version       # show version
linear config          # setup the project
linear completions     # generate shell completions
```
