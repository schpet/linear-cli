# initiative

> Manage Linear initiatives

## Usage

```
Usage:   linear initiative
Version: 1.8.1            

Description:

  Manage Linear initiatives

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

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
```

## Subcommands

### list

> List initiatives

```
Usage:   linear initiative list
Version: 1.8.1                 

Description:

  List initiatives

Options:

  -h, --help                 - Show this help.                                
  -w, --workspace  <slug>    - Target workspace (uses credentials)            
  -s, --status     <status>  - Filter by status (active, planned, completed)  
  --all-statuses             - Show all statuses (default: active only)       
  -o, --owner      <owner>   - Filter by owner (username or email)            
  -w, --web                  - Open initiatives page in web browser           
  -a, --app                  - Open initiatives page in Linear.app            
  -j, --json                 - Output as JSON                                 
  --archived                 - Include archived initiatives
```

### view

> View initiative details

```
Usage:   linear initiative view <initiativeId>
Version: 1.8.1                                

Description:

  View initiative details

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -w, --web                - Open in web browser                  
  -a, --app                - Open in Linear.app                   
  -j, --json               - Output as JSON
```

### create

> Create a new Linear initiative

```
Usage:   linear initiative create
Version: 1.8.1                   

Description:

  Create a new Linear initiative

Options:

  -h, --help                        - Show this help.                                        
  -w, --workspace    <slug>         - Target workspace (uses credentials)                    
  -n, --name         <name>         - Initiative name (required)                             
  -d, --description  <description>  - Initiative description                                 
  -s, --status       <status>       - Status: planned, active, completed (default: planned)  
  -o, --owner        <owner>        - Owner (username, email, or @me for yourself)           
  --target-date      <targetDate>   - Target completion date (YYYY-MM-DD)                    
  -c, --color        <color>        - Color hex code (e.g., #5E6AD2)                         
  --icon             <icon>         - Icon name                                              
  -i, --interactive                 - Interactive mode (default if no flags provided)        
  --no-color                        - Disable colored output
```

### archive

> Archive a Linear initiative

```
Usage:   linear initiative archive [initiativeId]
Version: 1.8.1                                   

Description:

  Archive a Linear initiative

Options:

  -h, --help                 - Show this help.                                    
  -w, --workspace  <slug>    - Target workspace (uses credentials)                
  -y, --force                - Skip confirmation prompt                           
  --no-color                 - Disable colored output                             
  --bulk           <ids...>  - Archive multiple initiatives by ID, slug, or name  
  --bulk-file      <file>    - Read initiative IDs from a file (one per line)     
  --bulk-stdin               - Read initiative IDs from stdin
```

### update

> Update a Linear initiative

```
Usage:   linear initiative update <initiativeId>
Version: 1.8.1                                  

Description:

  Update a Linear initiative

Options:

  -h, --help                        - Show this help.                                  
  -w, --workspace    <slug>         - Target workspace (uses credentials)              
  -n, --name         <name>         - New name for the initiative                      
  -d, --description  <description>  - New description                                  
  --status           <status>       - New status (planned, active, completed, paused)  
  --owner            <owner>        - New owner (username, email, or @me)              
  --target-date      <targetDate>   - Target completion date (YYYY-MM-DD)              
  --color            <color>        - Initiative color (hex, e.g., #5E6AD2)            
  --icon             <icon>         - Initiative icon name                             
  -i, --interactive                 - Interactive mode for updates                     
  --no-color                        - Disable colored output
```

### unarchive

> Unarchive a Linear initiative

```
Usage:   linear initiative unarchive <initiativeId>
Version: 1.8.1                                     

Description:

  Unarchive a Linear initiative

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -y, --force              - Skip confirmation prompt             
  --no-color               - Disable colored output
```

### delete

> Permanently delete a Linear initiative

```
Usage:   linear initiative delete [initiativeId]
Version: 1.8.1                                  

Description:

  Permanently delete a Linear initiative

Options:

  -h, --help                 - Show this help.                                   
  -w, --workspace  <slug>    - Target workspace (uses credentials)               
  -y, --force                - Skip confirmation prompt                          
  --no-color                 - Disable colored output                            
  --bulk           <ids...>  - Delete multiple initiatives by ID, slug, or name  
  --bulk-file      <file>    - Read initiative IDs from a file (one per line)    
  --bulk-stdin               - Read initiative IDs from stdin
```

### add-project

> Link a project to an initiative

```
Usage:   linear initiative add-project <initiative> <project>
Version: 1.8.1                                               

Description:

  Link a project to an initiative

Options:

  -h, --help                    - Show this help.                      
  -w, --workspace  <slug>       - Target workspace (uses credentials)  
  --sort-order     <sortOrder>  - Sort order within initiative         
  --no-color                    - Disable colored output
```

### remove-project

> Unlink a project from an initiative

```
Usage:   linear initiative remove-project <initiative> <project>
Version: 1.8.1                                                  

Description:

  Unlink a project from an initiative

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -y, --force              - Skip confirmation prompt             
  --no-color               - Disable colored output
```
