# initiative

> Manage Linear initiatives

## Usage

```
Usage:   linear initiative

Description:

  Manage Linear initiatives

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list, ls                                - List initiatives                      
  view, v         <initiativeId>          - View initiative details               
  create                                  - Create a new Linear initiative        
  archive         [initiativeId]          - Archive a Linear initiative           
  update          <initiativeId>          - Update a Linear initiative            
  unarchive       <initiativeId>          - Unarchive a Linear initiative         
  delete          [initiativeId]          - Permanently delete a Linear initiative
  add-project     <initiative> <project>  - Link a project to an initiative       
  remove-project  <initiative> <project>  - Unlink a project from an initiative   
  comment                                 - Manage initiative comments
```

## Subcommands

### add-project

> Link a project to an initiative

```
Usage:   linear initiative add-project <initiative> <project>

Description:

  Link a project to an initiative

Options:

  -h, --help                 - Show this help.                      
  --workspace   <slug>       - Target workspace (uses credentials)  
  --sort-order  <sortOrder>  - Sort order within initiative
```

### archive

> Archive a Linear initiative

```
Usage:   linear initiative archive [initiativeId]

Description:

  Archive a Linear initiative

Options:

  -h, --help              - Show this help.                                    
  --workspace   <slug>    - Target workspace (uses credentials)                
  -y, --force             - Skip confirmation prompt                           
  --bulk        <ids...>  - Archive multiple initiatives by ID, slug, or name  
  --bulk-file   <file>    - Read initiative IDs from a file (one per line)     
  --bulk-stdin            - Read initiative IDs from stdin
```

### comment

> Manage initiative comments

```
Usage:   linear initiative comment

Description:

  Manage initiative comments

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  add   <initiative>  - Add a comment or reply to an initiative's discussion (by ID, slug, or name)
  list  <initiative>  - List comments on an initiative (by ID, slug, or name)
```

#### comment subcommands

##### add

```
Usage:   linear initiative comment add <initiative>

Description:

  Add a comment or reply to an initiative's discussion (by ID, slug, or name)     
                                                                                  
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
Usage:   linear initiative comment list <initiative>

Description:

  List comments on an initiative (by ID, slug, or name)

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json           - Output as JSON
```

### create

> Create a new Linear initiative

```
Usage:   linear initiative create

Description:

  Create a new Linear initiative

Options:

  -h, --help                        - Show this help.                                        
  --workspace        <slug>         - Target workspace (uses credentials)                    
  -n, --name         <name>         - Initiative name (required)                             
  -d, --description  <description>  - Initiative description                                 
  -s, --status       <status>       - Status: planned, active, completed (default: planned)  
  -o, --owner        <owner>        - Owner (username, email, or @me for yourself)           
  --target-date      <targetDate>   - Target completion date (YYYY-MM-DD)                    
  -c, --color        <color>        - Color hex code (e.g., #5E6AD2)                         
  --icon             <icon>         - Icon name                                              
  -i, --interactive                 - Interactive mode (default if no flags provided)
```

### delete

> Permanently delete a Linear initiative

```
Usage:   linear initiative delete [initiativeId]

Description:

  Permanently delete a Linear initiative

Options:

  -h, --help              - Show this help.                                   
  --workspace   <slug>    - Target workspace (uses credentials)               
  -y, --force             - Skip confirmation prompt                          
  --bulk        <ids...>  - Delete multiple initiatives by ID, slug, or name  
  --bulk-file   <file>    - Read initiative IDs from a file (one per line)    
  --bulk-stdin            - Read initiative IDs from stdin
```

### list

> List initiatives

```
Usage:   linear initiative list

Description:

  List initiatives

Options:

  -h, --help                - Show this help.                                
  --workspace     <slug>    - Target workspace (uses credentials)            
  -s, --status    <status>  - Filter by status (active, planned, completed)  
  --all-statuses            - Show all statuses (default: active only)       
  -o, --owner     <owner>   - Filter by owner (username or email)            
  -w, --web                 - Open initiatives page in web browser           
  -a, --app                 - Open initiatives page in Linear.app            
  -j, --json                - Output as JSON                                 
  --archived                - Include archived initiatives
```

### remove-project

> Unlink a project from an initiative

```
Usage:   linear initiative remove-project <initiative> <project>

Description:

  Unlink a project from an initiative

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -y, --force          - Skip confirmation prompt
```

### unarchive

> Unarchive a Linear initiative

```
Usage:   linear initiative unarchive <initiativeId>

Description:

  Unarchive a Linear initiative

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -y, --force          - Skip confirmation prompt
```

### update

> Update a Linear initiative

```
Usage:   linear initiative update <initiativeId>

Description:

  Update a Linear initiative

Options:

  -h, --help                        - Show this help.                                  
  --workspace        <slug>         - Target workspace (uses credentials)              
  -n, --name         <name>         - New name for the initiative                      
  -d, --description  <description>  - New description                                  
  --status           <status>       - New status (planned, active, completed, paused)  
  --owner            <owner>        - New owner (username, email, or @me)              
  --target-date      <targetDate>   - Target completion date (YYYY-MM-DD)              
  --color            <color>        - Initiative color (hex, e.g., #5E6AD2)            
  --icon             <icon>         - Initiative icon name                             
  -i, --interactive                 - Interactive mode for updates
```

### view

> View initiative details

```
Usage:   linear initiative view <initiativeId>

Description:

  View initiative details

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -w, --web            - Open in web browser                  
  -a, --app            - Open in Linear.app                   
  -j, --json           - Output as JSON
```
