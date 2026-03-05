# project

> Manage Linear projects

## Usage

```
Usage:   linear project
Version: 1.10.0        

Description:

  Manage Linear projects

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                  - List projects                  
  view, v  <projectId>  - View project details           
  create                - Create a new Linear project    
  update   <projectId>  - Update a Linear project        
  delete   <projectId>  - Delete (trash) a Linear project
```

## Subcommands

### list

> List projects

```
Usage:   linear project list
Version: 1.10.0             

Description:

  List projects

Options:

  -h, --help                 - Show this help.                      
  -w, --workspace  <slug>    - Target workspace (uses credentials)  
  --team           <team>    - Filter by team key                   
  --all-teams                - Show projects from all teams         
  --status         <status>  - Filter by status name                
  -w, --web                  - Open in web browser                  
  -a, --app                  - Open in Linear.app                   
  -j, --json                 - Output as JSON
```

### view

> View project details

```
Usage:   linear project view <projectId>
Version: 1.10.0                         

Description:

  View project details

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -w, --web                - Open in web browser                  
  -a, --app                - Open in Linear.app
```

### create

> Create a new Linear project

```
Usage:   linear project create
Version: 1.10.0               

Description:

  Create a new Linear project

Options:

  -h, --help                        - Show this help.                                                          
  -w, --workspace    <slug>         - Target workspace (uses credentials)                                      
  -n, --name         <name>         - Project name (required)                                                  
  -d, --description  <description>  - Project description                                                      
  -t, --team         <team>         - Team key (required, can be repeated for multiple teams)                  
  -l, --lead         <lead>         - Project lead (username, email, or @me)                                   
  -s, --status       <status>       - Project status (planned, started, paused, completed, canceled, backlog)  
  --start-date       <startDate>    - Start date (YYYY-MM-DD)                                                  
  --target-date      <targetDate>   - Target completion date (YYYY-MM-DD)                                      
  --initiative       <initiative>   - Add to initiative immediately (ID, slug, or name)                        
  -i, --interactive                 - Interactive mode (default if no flags provided)                          
  -j, --json                        - Output created project as JSON
```

### update

> Update a Linear project

```
Usage:   linear project update <projectId>
Version: 1.10.0                           

Description:

  Update a Linear project

Options:

  -h, --help                        - Show this help.                                                  
  -w, --workspace    <slug>         - Target workspace (uses credentials)                              
  -n, --name         <name>         - Project name                                                     
  -d, --description  <description>  - Project description                                              
  -s, --status       <status>       - Status (planned, started, paused, completed, canceled, backlog)  
  -l, --lead         <lead>         - Project lead (username, email, or @me)                           
  --start-date       <startDate>    - Start date (YYYY-MM-DD)                                          
  --target-date      <targetDate>   - Target date (YYYY-MM-DD)                                         
  -t, --team         <team>         - Team key (can be repeated for multiple teams)
```

### delete

> Delete (trash) a Linear project

```
Usage:   linear project delete <projectId>
Version: 1.10.0                           

Description:

  Delete (trash) a Linear project

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -f, --force              - Skip confirmation prompt
```
