# linear cli

cli tool for linear.app that uses git branch names and directory names to open
issues and team pages. offers handy commands like `linear issue pr` to create
pull requests and `linear issue view` to open issues in linear.app.

## install

### from jsr

```bash
deno install --allow-env --allow-sys --allow-run --allow-read --allow-net -g -n linear jsr:@schpet/linear-cli
```

### local install

```bash
git clone https://github.com/schpet/linear-cli
cd linear
deno task install
```

## setup

### setup environment variables

first create an api key:

1. go to https://linear.app/settings/api
2. click "create key"
3. name it something like "cli"
4. copy the key

then export these environment variables:

```bash
export LINEAR_API_KEY="lin_api_..." # paste your api key here
export LINEAR_WORKSPACE="your-company" # your linear.app workspace url slug
```

## repo setup

- name your repos with your linear team prefix, i.e.
  `mv ~/code/cool-project ~/code/eng-cool-project`
- use branch names prefixed with a linear issue id, note that
  [linear's github integration](https://linear.app/docs/github#branch-format)
  will suggest these branch names

## commands

### issue commands

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
