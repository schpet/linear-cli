# auth

> Manage Linear authentication

## Usage

```
Usage:   linear auth
Version: 1.8.1      

Description:

  Manage Linear authentication

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  login                 - Add a workspace credential                    
  logout   [workspace]  - Remove a workspace credential                 
  list                  - List configured workspaces                    
  default  [workspace]  - Set the default workspace                     
  token                 - Print the configured API token                
  whoami                - Print information about the authenticated user
```

## Subcommands

### login

> Add a workspace credential

```
Usage:   linear auth login
Version: 1.8.1            

Description:

  Add a workspace credential

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -k, --key        <key>   - API key (prompted if not provided)
```

### logout

> Remove a workspace credential

```
Usage:   linear auth logout [workspace]
Version: 1.8.1                         

Description:

  Remove a workspace credential

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -f, --force              - Skip confirmation prompt
```

### list

> List configured workspaces

```
Usage:   linear auth list
Version: 1.8.1           

Description:

  List configured workspaces

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### default

> Set the default workspace

```
Usage:   linear auth default [workspace]
Version: 1.8.1                          

Description:

  Set the default workspace

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### token

> Print the configured API token

```
Usage:   linear auth token
Version: 1.8.1            

Description:

  Print the configured API token

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### whoami

> Print information about the authenticated user

```
Usage:   linear auth whoami
Version: 1.8.1             

Description:

  Print information about the authenticated user

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```
