# initiative-update

> Manage initiative status updates (timeline posts)

## Usage

```
Usage:   linear initiative-update
Version: 1.7.0                   

Description:

  Manage initiative status updates (timeline posts)

Options:

  -h, --help  - Show this help.  

Commands:

  create, c    <initiativeId>  - Create a new status update for an initiative
  list, l, ls  <initiativeId>  - List status updates for an initiative
```

## Subcommands

### create

> Create a new status update for an initiative

```
Usage:   linear initiative-update create <initiativeId>
Version: 1.7.0                                         

Description:

  Create a new status update for an initiative

Options:

  -h, --help                   - Show this help.                            
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
Version: 1.7.0                                       

Description:

  List status updates for an initiative

Options:

  -h, --help           - Show this help.               
  -j, --json           - Output as JSON                
  --limit     <limit>  - Limit results    (Default: 10)
```
