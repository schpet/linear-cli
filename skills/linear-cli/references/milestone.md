# milestone

> Manage Linear project milestones

## Usage

```
Usage:   linear milestone

Description:

  Manage Linear project milestones

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                    - List milestones for a project       
  view, v  <milestoneId>  - View milestone details              
  create                  - Create a new project milestone      
  update   <id>           - Update an existing project milestone
  delete   <id>           - Delete a project milestone
```

## Subcommands

### list

> List milestones for a project

```
Usage:   linear milestone list --project <projectId>

Description:

  List milestones for a project

Options:

  -h, --help                    - Show this help.                                
  -w, --workspace  <slug>       - Target workspace (uses credentials)            
  --project        <projectId>  - Project ID                           (required)
```

### view

> View milestone details

```
Usage:   linear milestone view <milestoneId>

Description:

  View milestone details

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### create

> Create a new project milestone

```
Usage:   linear milestone create --project <projectId> --name <name>

Description:

  Create a new project milestone

Options:

  -h, --help                      - Show this help.                                
  -w, --workspace  <slug>         - Target workspace (uses credentials)            
  --project        <projectId>    - Project ID                           (required)
  --name           <name>         - Milestone name                       (required)
  --description    <description>  - Milestone description                          
  --target-date    <date>         - Target date (YYYY-MM-DD)
```

### update

> Update an existing project milestone

```
Usage:   linear milestone update <id>

Description:

  Update an existing project milestone

Options:

  -h, --help                      - Show this help.                          
  -w, --workspace  <slug>         - Target workspace (uses credentials)      
  --name           <name>         - Milestone name                           
  --description    <description>  - Milestone description                    
  --target-date    <date>         - Target date (YYYY-MM-DD)                 
  --sort-order     <value>        - Sort order relative to other milestones  
  --project        <projectId>    - Move to a different project
```

### delete

> Delete a project milestone

```
Usage:   linear milestone delete <id>

Description:

  Delete a project milestone

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -f, --force              - Skip confirmation prompt
```
