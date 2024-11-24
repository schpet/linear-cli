# linear cli

cli tool for linear.app that uses git branch names and directory names to open
issues and team pages.

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

## usage

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

### team pages

name your project directories with your linear team prefix:

```bash
mkdir eng-project # eng is the team prefix
cd eng-project
linear team # opens eng team page
```

### issues

create branches that include linear issue ids:

```bash
git switch -c eng-123-do-something
linear issue # opens eng-123 in linear
linear issue print # prints issue title and description
```

note that
[linear's github integration](https://linear.app/docs/github#branch-format) will
suggest these branch names.

## commands

Here are all available commands and their usage:

### Team Commands

```bash
linear team              # opens team page in Linear.app
linear team open        # same as above
linear team id          # prints the team id (e.g., "ENG")
linear team autolinks   # configures GitHub repository autolinks for Linear issues
```

### Issue Commands

```bash
linear issue            # opens current branch's issue in Linear.app
linear issue open      # same as above
linear issue print     # prints issue title and description (with markdown formatting)
linear issue print --no-color  # prints issue without color/formatting
linear issue id        # prints the issue id from current branch (e.g., "ENG-123")
linear issue title     # prints just the issue title
linear issue url       # prints the Linear.app URL for the issue
linear issue pr        # creates a GitHub PR with issue details
linear issue pull-request  # same as above
```

### Other Commands

```bash
linear --help          # show all commands
linear --version       # show version
linear completions     # generate shell completions
```
