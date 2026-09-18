# team

> Manage Linear teams

## Usage

```
Usage:   linear team

Description:

  Manage Linear teams

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  create             - Create a linear team                                                         
  delete     <team>  - Delete a Linear team                                                         
  list               - List teams                                                                   
  id                 - Print the configured team id                                                 
  autolinks          - Configure GitHub repository autolinks for Linear issues with this team prefix
  members    [team]  - List team members (team by key, name, or ID)                                 
  states     [team]  - List workflow states for a team (by key, name, or ID)
```

## Subcommands

### autolinks

> Configure GitHub repository autolinks for Linear issues with this team prefix

```
Usage:   linear team autolinks

Description:

  Configure GitHub repository autolinks for Linear issues with this team prefix

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)
```

### create

> Create a linear team

```
Usage:   linear team create

Description:

  Create a linear team

Options:

  -h, --help                        - Show this help.                                          
  --workspace        <slug>         - Target workspace (uses credentials)                      
  -n, --name         <name>         - Name of the team                                         
  -d, --description  <description>  - Description of the team                                  
  -k, --key          <key>          - Team key (if not provided, will be generated from name)  
  --private                         - Make the team private                                    
  --no-interactive                  - Disable interactive prompts
```

### delete

> Delete a Linear team

```
Usage:   linear team delete <team>

Description:

  Delete a Linear team

Options:

  -h, --help                   - Show this help.                                                     
  --workspace    <slug>        - Target workspace (uses credentials)                                 
  --move-issues  <targetTeam>  - Move all issues to another team (key, name, or ID) before deletion  
  -y, --force                  - Skip confirmation prompt
```

### id

> Print the configured team id

```
Usage:   linear team id

Description:

  Print the configured team id

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)
```

### list

> List teams

```
Usage:   linear team list

Description:

  List teams

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -w, --web            - Open in web browser                  
  -a, --app            - Open in Linear.app                   
  -j, --json           - Output as JSON
```

### members

> List team members (team by key, name, or ID)

```
Usage:   linear team members [team]

Description:

  List team members (team by key, name, or ID)

Options:

  -h, --help           - Show this help.                                                         
  --workspace  <slug>  - Target workspace (uses credentials)                                     
  -a, --all            - Include inactive members                                                
  -j, --json           - Output as JSON; a member's url mentions them when pasted into Markdown
```

### states

> List workflow states for a team (by key, name, or ID)

```
Usage:   linear team states [team]

Description:

  List workflow states for a team (by key, name, or ID)

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json           - Output as JSON
```
