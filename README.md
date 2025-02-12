# linear cli

linear's UI is incredibly good but it can't take advantage of me already being in my project, on the command line. e.g. 

- linear makes me switch context from where i'm working to their app or website
- linear doesn't know what team i'm currently acting on, so i have to navigate to it
- linear can suggest a git branch, but it makes me do the work of creating or switching to that branch
- linear's suggested git branch doesn't account for it already existing and having a merged pull request

this cli solves this: it knows what you're working on, does the work of managing branches, and will write your pull request details for you.

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

Environment variables can be set in your shell or in a `.env` file. The CLI will
look for a `.env` file in:

1. Your current working directory
2. The root of your git repository (if you're in a git repo)

Example `.env` file:

```sh
LINEAR_API_KEY="lin_api_..." # create an api key at https://linear.app/settings/api (requires member access, not available for guest accounts)
LINEAR_WORKSPACE="your-company" # your linear workspace url slug
LINEAR_TEAM_ID="ENG" # optional: set team id instead of using directory name
LINEAR_ISSUE_SORT="priority" # optional: default sort order for issue list (priority|manual)
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
