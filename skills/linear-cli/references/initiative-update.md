# initiative-update

> Manage initiative status updates (timeline posts)

## Usage

```
Usage:   linear initiative-update

Description:

  Manage initiative status updates (timeline posts)

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  create, c    <initiativeId>  - Create a new status update for an initiative
  list, l, ls  <initiativeId>  - List status updates for an initiative
```

## Subcommands

### create

> Create a new status update for an initiative

```
Usage:   linear initiative-update create <initiativeId>

Description:

  Create a new status update for an initiative                                    
                                                                                  
  Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,  
  and `[Name](url)` do not. Get a person's URL from the `url` field of            
  `linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
  Run `linear markdown` for collapsible sections and the full reference.          

Options:

  -h, --help                   - Show this help.                            
  --workspace        <slug>    - Target workspace (uses credentials)        
  --body             <body>    - Update content (markdown)                  
  --body-file        <path>    - Read content from file                     
  --health           <health>  - Health status (onTrack, atRisk, offTrack)  
  -i, --interactive            - Interactive mode with prompts
```

### list

> List status updates for an initiative

```
Usage:   linear initiative-update list <initiativeId>

Description:

  List status updates for an initiative

Options:

  -h, --help            - Show this help.                                   
  --workspace  <slug>   - Target workspace (uses credentials)               
  -j, --json            - Output as JSON                                    
  --limit      <limit>  - Limit results                        (Default: 10)
```
