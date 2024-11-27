# linear cli

cli tool for linear.app that uses git branch names and directory names to open
issues and team pages. offers handy commands:

- `linear issue` open the issue in linear.app, based on the current git branch
- `linear issue print` print it to stdout
- `linear issue pr` create a nicely named pull request with
  [gh](https://cli.github.com/)
- `linear team` view active issues assigned to you in the team

see [the full list of commands](#commands) below.

## install

### homebrew

todo

### deno via jsr

```bash
deno install --allow-env --allow-sys --allow-run --allow-read --allow-net -g -n linear jsr:@schpet/linear-cli
```

### deno via local git clone

```bash
git clone https://github.com/schpet/linear-cli
cd linear-cli
deno task install
```

## setup

### required environment variables

```bash
export LINEAR_API_KEY="lin_api_..." # create an api key at https://linear.app/settings/api
export LINEAR_WORKSPACE="your-company" # your linear.app workspace url slug
```

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
linear issue print --no-color  # prints issue without color/formatting
linear issue id        # prints the issue id from current branch (e.g., "ENG-123")
linear issue title     # prints just the issue title
linear issue url       # prints the Linear.app URL for the issue
linear issue pr        # creates a GitHub PR with issue details via `gh pr create`
linear issue pull-request  # same as above
```

### team commands

name your repo directories with your project's linear team prefix for these to
work. i.e. if your linear team prefix is 'ENG'
`mv ~/code/cool-project ~/code/eng-cool-project`

```bash
linear team              # opens team page in Linear.app
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
