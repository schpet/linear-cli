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
  comment               - Manage project comments
```

## Subcommands

### comment

> Manage project comments

```
Usage:   linear project comment

Description:

  Manage project comments

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  add   <project>  - Add a comment or reply to a project's discussion (by ID, slug, or name)
  list  <project>  - List comments on a project (by ID, slug, or name)
```

#### comment subcommands

##### add

```
Usage:   linear project comment add <project>

Description:

  Add a comment or reply to a project's discussion (by ID, slug, or name)         
                                                                                  
  Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,  
  and `[Name](url)` do not. Get a person's URL from the `url` field of            
  `linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
  Run `linear markdown` for collapsible sections and the full reference.          

Options:

  -h, --help                             - Show this help.                                                   
  --workspace               <slug>       - Target workspace (uses credentials)                               
  -b, --body                <text>       - Comment body text                                                 
  --body-file               <path>       - Read comment body from a file (preferred for markdown content)    
  -p, --parent, --reply-to  <commentId>  - Reply to a top-level comment by ID (the reply joins that thread)
```

##### list

```
Usage:   linear project comment list <project>

Description:

  List comments on a project (by ID, slug, or name)

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json           - Output as JSON
```

### create

> Create a new Linear project

```
Usage:   linear project create

Description:

  Create a new Linear project                                                     
                                                                                  
  Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,  
  and `[Name](url)` do not. Get a person's URL from the `url` field of            
  `linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
  Run `linear markdown` for collapsible sections and the full reference.          

Options:

  -h, --help                             - Show this help.                                                             
  --workspace             <slug>         - Target workspace (uses credentials)                                         
  -n, --name              <name>         - Project name (required)                                                     
  -d, --description       <description>  - Project description (max 255 characters, enforced by Linear's API)          
  -f, --description-file  <path>         - Read project description from file (still subject to the 255-character API  
                                           limit)                                                                      
  --content               <markdown>     - Project overview markdown                                                   
  --content-file          <path>         - Read project overview markdown from a file                                  
  -t, --team              <team>         - Team key, name, or ID (required, can be repeated for multiple teams)        
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
  --team       <team>    - Filter by team key, name, or ID      
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
                                                                                  
  Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,  
  and `[Name](url)` do not. Get a person's URL from the `url` field of            
  `linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
  Run `linear markdown` for collapsible sections and the full reference.          

Options:

  -h, --help                             - Show this help.                                                             
  --workspace             <slug>         - Target workspace (uses credentials)                                         
  -n, --name              <name>         - Project name                                                                
  -d, --description       <description>  - Project description (max 255 characters, enforced by Linear's API)          
  -f, --description-file  <path>         - Read project description from file (still subject to the 255-character API  
                                           limit)                                                                      
  --content               <markdown>     - Project overview markdown                                                   
  --content-file          <path>         - Read project overview markdown from a file                                  
  -s, --status            <status>       - Status (planned, started, paused, completed, canceled, backlog)             
  -l, --lead              <lead>         - Project lead (username, email, or @me). Use --clear-lead to remove it       
  --clear-lead                           - Remove the project's lead (cannot be combined with --lead)                  
  --start-date            <startDate>    - Start date (YYYY-MM-DD). Use --clear-start-date to remove it                
  --clear-start-date                     - Remove the project's start date (cannot be combined with --start-date)      
  --target-date           <targetDate>   - Target date (YYYY-MM-DD). Use --clear-target-date to remove it              
  --clear-target-date                    - Remove the project's target date (cannot be combined with --target-date)    
  -t, --team              <team>         - Team key, name, or ID (can be repeated for multiple teams)                  
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
  -j, --json           - Output as JSON
```
