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

### environment variables

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

```bash
linear --help # show all commands
linear issue --help # show issue commands
linear team --help # show team commands
```
