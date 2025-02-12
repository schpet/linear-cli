# linear cli

a command line interface for Linear that integrates with your development workflow.

Linear's UI is great, but sometimes switching context to the web/desktop app breaks your flow. this cli helps by:

- keeping you in your terminal where you're already working
- knowing which team you're working with based on your project
- managing git branches automatically
- handling existing branches and merged pull requests

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

1. Create an API key at https://linear.app/settings/api (requires member access, not available for guest accounts)

2. Add the API key to your shell environment:

   ```sh
   # in ~/.bashrc or ~/.zshrc:
   export LINEAR_API_KEY="lin_api_..."
   
   # or in fish:
   set -Ux LINEAR_API_KEY "lin_api_..."
   ```

3. Run the configuration wizard:

   ```sh
   linear config
   ```

   This will create a `.linear.toml` config file in your repository with your workspace and team settings.

The CLI works best when your git branches include Linear issue IDs (e.g. `eng-123-my-feature`). Linear's 'copy git branch name' button and [related automations](https://linear.app/docs/account-preferences#git-related-automations) can help with this.

## commands

### issue commands

the current issue is determined by the issue id in the current git branch name.
note that [Linear's GitHub integration](https://linear.app/docs/github#branch-format) will suggest these branch names.

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

name your repo directories with your project's Linear team prefix for these to
work. i.e. if your team prefix is 'ENG':
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
