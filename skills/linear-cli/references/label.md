# label

> Manage Linear issue labels

## Usage

```
Usage:   linear label
Version: 1.8.1       

Description:

  Manage Linear issue labels

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                - List issue labels       
  create              - Create a new issue label
  delete  <nameOrId>  - Delete an issue label
```

## Subcommands

### list

> List issue labels

```
Usage:   linear label list
Version: 1.8.1            

Description:

  List issue labels

Options:

  -h, --help              - Show this help.                                              
  --team       <teamKey>  - Filter by team (e.g., TC). Shows team-specific labels only.  
  --workspace             - Show only workspace-level labels (not team-specific)         
  --all                   - Show all labels (both workspace and team)                    
  -j, --json              - Output as JSON
```

### create

> Create a new issue label

```
Usage:   linear label create
Version: 1.8.1              

Description:

  Create a new issue label

Options:

  -h, --help                        - Show this help.                                              
  -w, --workspace    <slug>         - Target workspace (uses credentials)                          
  -n, --name         <name>         - Label name (required)                                        
  -c, --color        <color>        - Color hex code (e.g., #EB5757)                               
  -d, --description  <description>  - Label description                                            
  -t, --team         <teamKey>      - Team key for team-specific label (omit for workspace label)  
  -i, --interactive                 - Interactive mode (default if no flags provided)
```

### delete

> Delete an issue label

```
Usage:   linear label delete <nameOrId>
Version: 1.8.1                         

Description:

  Delete an issue label

Options:

  -h, --help                  - Show this help.                                 
  -w, --workspace  <slug>     - Target workspace (uses credentials)             
  -t, --team       <teamKey>  - Team key to disambiguate labels with same name  
  -f, --force                 - Skip confirmation prompt
```
