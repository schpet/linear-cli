# team

> Manage Linear teams

## Usage

```
Usage:   linear team
Version: 1.8.1      

Description:

  Manage Linear teams

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  create                - Create a linear team                                                         
  delete     <teamKey>  - Delete a Linear team                                                         
  list                  - List teams                                                                   
  id                    - Print the configured team id                                                 
  autolinks             - Configure GitHub repository autolinks for Linear issues with this team prefix
  members    [teamKey]  - List team members
```

## Subcommands

### create

> Create a linear team

```
Usage:   linear team create
Version: 1.8.1             

Description:

  Create a linear team

Options:

  -h, --help                        - Show this help.                                          
  -w, --workspace    <slug>         - Target workspace (uses credentials)                      
  -n, --name         <name>         - Name of the team                                         
  -d, --description  <description>  - Description of the team                                  
  -k, --key          <key>          - Team key (if not provided, will be generated from name)  
  --private                         - Make the team private                                    
  --no-color                        - Disable colored output                                   
  --no-interactive                  - Disable interactive prompts
```

### delete

> Delete a Linear team

```
Usage:   linear team delete <teamKey>
Version: 1.8.1                       

Description:

  Delete a Linear team

Options:

  -h, --help                     - Show this help.                                  
  -w, --workspace  <slug>        - Target workspace (uses credentials)              
  --move-issues    <targetTeam>  - Move all issues to another team before deletion  
  -y, --force                    - Skip confirmation prompt
```

### list

> List teams

```
Usage:   linear team list
Version: 1.8.1           

Description:

  List teams

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -w, --web                - Open in web browser                  
  -a, --app                - Open in Linear.app
```

### id

> Print the configured team id

```
Usage:   linear team id
Version: 1.8.1         

Description:

  Print the configured team id

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### autolinks

> Configure GitHub repository autolinks for Linear issues with this team prefix

```
Usage:   linear team autolinks
Version: 1.8.1                

Description:

  Configure GitHub repository autolinks for Linear issues with this team prefix

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### members

> List team members

```
Usage:   linear team members [teamKey]
Version: 1.8.1                        

Description:

  List team members

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -a, --all                - Include inactive members
```
