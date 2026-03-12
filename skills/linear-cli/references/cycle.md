# cycle

> Manage Linear team cycles

## Usage

```
Usage:   linear cycle

Description:

  Manage Linear team cycles

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                 - List cycles for a team                  
  view, v  <cycleRef>  - View cycle details                      
  current              - Show the current active cycle for a team
  add      <issueId>   - Add an issue to a cycle
```

## Subcommands

### list

> List cycles for a team

```
Usage:   linear cycle list

Description:

  List cycles for a team

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  --team           <team>  - Team key (defaults to current team)
```

### view

> View cycle details

```
Usage:   linear cycle view <cycleRef>

Description:

  View cycle details

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  --team           <team>  - Team key (defaults to current team)
```

### current

> Show the current active cycle for a team

```
Usage:   linear cycle current

Description:

  Show the current active cycle for a team

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  --team           <team>  - Team key (defaults to current team)
```

### add

> Add an issue to a cycle

```
Usage:   linear cycle add <issueId>

Description:

  Add an issue to a cycle

Options:

  -h, --help                - Show this help.                                                     
  -w, --workspace  <slug>   - Target workspace (uses credentials)                                 
  --team           <team>   - Team key (defaults to current team)                                 
  --cycle          <cycle>  - Cycle name or number (defaults to active cycle)  (Default: "active")
```
