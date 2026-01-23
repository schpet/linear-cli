# milestone

> Manage Linear project milestones

## Usage

```
Usage:   linear milestone
Version: 1.8.1           

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
Version: 1.8.1                                      

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
Version: 1.8.1                              

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
Version: 1.8.1                                                      

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
Version: 1.8.1                       

Description:

  Update an existing project milestone

Options:

  -h, --help                      - Show this help.                      
  -w, --workspace  <slug>         - Target workspace (uses credentials)  
  --name           <name>         - Milestone name                       
  --description    <description>  - Milestone description                
  --target-date    <date>         - Target date (YYYY-MM-DD)             
  --project        <projectId>    - Move to a different project
```

### delete

> Delete a project milestone

```
Usage:   linear milestone delete <id>
Version: 1.8.1                       

Description:

  Delete a project milestone

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -f, --force              - Skip confirmation prompt
```
