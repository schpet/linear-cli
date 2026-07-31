# project

> Manage Linear projects

## Usage

```
Usage:   linear project

Description:

  Manage Linear projects

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                  - List projects                  
  view, v  <projectId>  - View project details           
  create                - Create a new Linear project    
  update   <projectId>  - Update a Linear project        
  delete   <projectId>  - Delete (trash) a Linear project
```

## Subcommands

### create

> Create a new Linear project

```
Usage:   linear project create

Description:

  Create a new Linear project

Options:

  -h, --help                             - Show this help.                                                             
  --workspace             <slug>         - Target workspace (uses credentials)                                         
  -n, --name              <name>         - Project name (required)                                                     
  -d, --description       <description>  - Project description (max 255 characters, enforced by Linear's API)          
  -f, --description-file  <path>         - Read project description from file (still subject to the 255-character API  
                                           limit)                                                                      
  --content               <markdown>     - Project overview markdown                                                   
  --content-file          <path>         - Read project overview markdown from a file                                  
  -t, --team              <team>         - Team key (required, can be repeated for multiple teams)                     
  -l, --lead              <lead>         - Project lead (username, email, or @me)                                      
  -s, --status            <status>       - Project status (planned, started, paused, completed, canceled, backlog)     
  --start-date            <startDate>    - Start date (YYYY-MM-DD)                                                     
  --target-date           <targetDate>   - Target completion date (YYYY-MM-DD)                                         
  --priority              <priority>     - Project priority (none, urgent, high, medium, low)                          
  --label                 <label>        - Project label associated with the project. May be repeated.                 
  --member                <user>         - Project member (username, email, display name, or @me). May be repeated.    
  --icon                  <icon>         - Project icon                                                                
  --color                 <color>        - Project color as a HEX string                                               
  --initiative            <initiative>   - Add to initiative immediately (ID, slug, or name)                           
  -i, --interactive                      - Interactive mode (default if no flags provided)                             
  -j, --json                             - Output created project as JSON
```

### delete

> Delete (trash) a Linear project

```
Usage:   linear project delete <projectId>

Description:

  Delete (trash) a Linear project

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -f, --force          - Skip confirmation prompt
```

### list

> List projects

```
Usage:   linear project list

Description:

  List projects

Options:

  -h, --help             - Show this help.                      
  --workspace  <slug>    - Target workspace (uses credentials)  
  --team       <team>    - Filter by team key                   
  --all-teams            - Show projects from all teams         
  --status     <status>  - Filter by status name                
  -w, --web              - Open in web browser                  
  -a, --app              - Open in Linear.app                   
  -j, --json             - Output as JSON
```

### update

> Update a Linear project

```
Usage:   linear project update <projectId>

Description:

  Update a Linear project

Options:

  -h, --help                             - Show this help.                                                             
  --workspace             <slug>         - Target workspace (uses credentials)                                         
  -n, --name              <name>         - Project name                                                                
  -d, --description       <description>  - Project description (max 255 characters, enforced by Linear's API)          
  -f, --description-file  <path>         - Read project description from file (still subject to the 255-character API  
                                           limit)                                                                      
  -s, --status            <status>       - Status (planned, started, paused, completed, canceled, backlog)             
  -l, --lead              <lead>         - Project lead (username, email, or @me)                                      
  --start-date            <startDate>    - Start date (YYYY-MM-DD)                                                     
  --target-date           <targetDate>   - Target date (YYYY-MM-DD)                                                    
  -t, --team              <team>         - Team key (can be repeated for multiple teams)                               
  --label                 <label>        - Replace the project's labels. May be repeated to set multiple labels.
```

### view

> View project details

```
Usage:   linear project view <projectId>

Description:

  View project details

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -w, --web            - Open in web browser                  
  -a, --app            - Open in Linear.app
```
