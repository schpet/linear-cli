# initiative-update

> Manage initiative status updates (timeline posts)

## Usage

```
Usage:   linear initiative-update
Version: 1.8.1                   

Description:

  Manage initiative status updates (timeline posts)

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  create, c    <initiativeId>  - Create a new status update for an initiative
  list, l, ls  <initiativeId>  - List status updates for an initiative
```

## Subcommands

### create

> Create a new status update for an initiative

```
Usage:   linear initiative-update create <initiativeId>
Version: 1.8.1                                         

Description:

  Create a new status update for an initiative

Options:

  -h, --help                   - Show this help.                            
  -w, --workspace    <slug>    - Target workspace (uses credentials)        
  --body             <body>    - Update content (markdown)                  
  --body-file        <path>    - Read content from file                     
  --health           <health>  - Health status (onTrack, atRisk, offTrack)  
  -i, --interactive            - Interactive mode with prompts              
  --no-color                   - Disable colored output
```

### list

> List status updates for an initiative

```
Usage:   linear initiative-update list <initiativeId>
Version: 1.8.1                                       

Description:

  List status updates for an initiative

Options:

  -h, --help                - Show this help.                                   
  -w, --workspace  <slug>   - Target workspace (uses credentials)               
  -j, --json                - Output as JSON                                    
  --limit          <limit>  - Limit results                        (Default: 10)
```
