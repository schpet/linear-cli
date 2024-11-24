# linear cli

cli tool for linear.app that uses git branch names and directory names to open issues and team pages.

## install

### from jsr

```bash
deno install jsr:@schpet/linear
```

### local install

```bash
git clone https://github.com/schpet/linear
cd linear
deno task install
```

## usage

### environment variables

```bash
export LINEAR_API_KEY="lin_api_..." # get this from linear.app settings
export LINEAR_WORKSPACE="your-workspace" # your linear.app workspace name
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
git checkout -b eng-123-do-something
linear issue # opens eng-123 in linear
linear issue print # prints issue title and description
```

## commands

```bash
linear --help # show all commands
linear issue --help # show issue commands
linear team --help # show team commands
```
