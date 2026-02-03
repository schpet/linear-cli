# project

> Manage Linear projects

## Usage

```
Usage:   linear project
Version: 1.8.1         

Description:

  Manage Linear projects

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                  - List projects              
  view, v  <projectId>  - View project details       
  create                - Create a new Linear project
```

## Subcommands

### list

> List projects

```
Usage:   linear project list
Version: 1.8.1              

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
```

### view

> View project details

```
Usage:   linear project view <projectId>
Version: 1.8.1                          

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
Version: 1.8.1                

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
  --no-color                        - Disable colored output
```
