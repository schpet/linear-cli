# linear cli

cli tool for linear.app that uses git branch names and directory names to open
issues and team pages. offers handy commands:

| command            | description                                                           |
| ------------------ | --------------------------------------------------------------------- |
| linear issue       | open the issue in linear.app, based on the current git branch         |
| linear issue print | print the issue to stdout                                             |
| linear issue pr    | create a nicely named pull request with [gh](https://cli.github.com/) |
| linear issue list  | list your issues in a table view with sorting and filtering           |
| linear team        | view active issues assigned to you in the team                        |

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

this cli needs three things to work:

1. a couple environment variables, see below
2. your current git branch to start with a linear issue id, e.g.
   `eng-123-my-feature`, this complements linear's 'copy git branch name' button
   and its
   [related automations](https://linear.app/docs/account-preferences#git-related-automations)
3. either:
   - the directory of your repo to start with a linear team id\
     e.g. if your team's identifier is `ENG` and your repo is at
     `~/code/cool-proj` you'll need to
     ```sh
     mv ~/code/cool-proj ~/code/eng-cool-proj
     ```
   - or set LINEAR_TEAM_ID environment variable (useful with tools like direnv)
     ```sh
     LINEAR_TEAM_ID=ENG
     ```

### required environment variables

```sh
LINEAR_API_KEY="lin_api_..." # create an api key at https://linear.app/settings/api (requires member access, not available for guest accounts)
LINEAR_WORKSPACE="your-company" # your linear workspace url slug
LINEAR_TEAM_ID="ENG" # optional: set team id instead of using directory name
```

<details>
<summary>bash</summary>

add to ~/.bashrc:

```sh
# secrets! make sure this file isn't shared online
export LINEAR_API_KEY="lin_api_..."
export LINEAR_WORKSPACE="your-company"
```

</details>

<details>
<summary>zsh</summary>

add to ~/.zshrc:

```sh
# secrets! make sure this file isn't shared online
export LINEAR_API_KEY="lin_api_..."
export LINEAR_WORKSPACE="your-company"
```

</details>

<details>
<summary>fish</summary>

run in terminal:

```sh
# secrets! make sure ~/.config/fish/fish_variables isn't shared online
set -Ux LINEAR_API_KEY "lin_api_..."
set -Ux LINEAR_WORKSPACE "your-company"
```

</details>

## commands

### issue commands

the current issue is determined by the issue id in the current git branch name.
note that
[linear's github integration](https://linear.app/docs/github#branch-format) will
suggest these branch names.

```bash
linear issue           # opens current branch's issue in Linear.app
linear issue open      # same as above
linear issue print     # prints issue title and description (with markdown formatting)
linear issue id        # prints the issue id from current branch (e.g., "ENG-123")
linear issue title     # prints just the issue title
linear issue url       # prints the Linear.app URL for the issue
linear issue pr        # creates a GitHub PR with issue details via `gh pr create`
linear issue list      # list your issues in a table view (supports --sort and --state)
```

### team commands

name your repo directories with your project's linear team prefix for these to
work. i.e. if your linear team prefix is 'ENG'
`mv ~/code/cool-project ~/code/eng-cool-project`

```bash
linear team             # opens team page in Linear.app
linear team open        # same as above
linear team id          # prints the team id (e.g., "ENG")
linear team autolinks   # configures GitHub repository autolinks for Linear issues
```

### other commands

```bash
linear --help          # show all commands
linear --version       # show version
linear completions     # generate shell completions
```
