# project-update

> Manage project status updates

## Usage

```
Usage:   linear project-update
Version: 1.8.1                

Description:

  Manage project status updates

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  create, c  <projectId>  - Create a new status update for a project
  list, l    <projectId>  - List status updates for a project
```

## Subcommands

### create

> Create a new status update for a project

```
Usage:   linear project-update create <projectId>
Version: 1.8.1                                   

Description:

  Create a new status update for a project

Options:

  -h, --help                   - Show this help.                                    
  -w, --workspace    <slug>    - Target workspace (uses credentials)                
  --body             <body>    - Update content (inline)                            
  --body-file        <path>    - Read content from file                             
  --health           <health>  - Project health status (onTrack, atRisk, offTrack)  
  -i, --interactive            - Interactive mode with prompts                      
  --no-color                   - Disable colored output
```

### list

> List status updates for a project

```
Usage:   linear project-update list <projectId>
Version: 1.8.1                                 

Description:

  List status updates for a project

Options:

  -h, --help                - Show this help.                                   
  -w, --workspace  <slug>   - Target workspace (uses credentials)               
  --json                    - Output as JSON                                    
  --limit          <limit>  - Limit results                        (Default: 10)
```
