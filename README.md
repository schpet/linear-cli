# linear cli

a cli to list, start and create issues. git aware to keep you in the right views
in linear. allows jumping to the web or the linear desktop app similar to `gh`.

here's how it works:

```bash
linear config               # setup your repo, it writes a config file

linear issue list           # list issues assigned to you
linear issue start ABC-123  # update an issue status, checks out a branch
linear issue pr             # makes a PR with title/body preset, leverages gh (https://cli.github.com/)
linear issue create         # create a new issue
```

it's pretty dialed to my own use cases, but i want to support more people than
myself with this so
[lmk what it can do for you](https://github.com/schpet/linear-cli/issues/).

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

1. create an API key at https://linear.app/settings/account/security\
   _fyi creating an api key requires member access, it is not available for
   guest accounts_

2. add the API key to your shell environment:

   ```sh
   # in ~/.bashrc or ~/.zshrc:
   export LINEAR_API_KEY="lin_api_..."

   # or in fish:
   set -Ux LINEAR_API_KEY "lin_api_..."
   ```

3. run the configuration wizard:

   ```sh
   cd my-project-repo
   linear config
   ```

   _this will create a `.linear.toml` config file in your repository with your
   workspace and team settings._

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
linear issue start     # create/switch to issue branch and mark as started
linear issue create    # create a new issue (interactive prompts)
linear issue create -t "title" -d "description"  # create with flags

linear team id         # print out the team id (e.g. for scripts)
linear autolinks       # use gh to setup autolinked references from github to linear
```

### other commands

```bash
linear --help          # show all commands
linear --version       # show version
linear config          # setup the project
linear completions     # generate shell completions
```

## why

linear's UI is incredibly good but it slows me down. i find the following pretty
grating to experience frequently:

- switching context from my repo to linear
- not being on the right view when i open linear
- linear suggests a git branch, but i have to do the work of creating or
  switching to that branch
- linear's suggested git branch doesn't account for it already existing or
  having a merged pull request

this cli solves this. it knows what you're working on, does the work of managing
branches, and will write your pull request details for you.
