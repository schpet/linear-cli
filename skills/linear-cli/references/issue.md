# issue

> Manage Linear issues

## Usage

```
Usage:   linear issue
Version: 1.10.0      

Description:

  Manage Linear issues

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  id                                      - Print the issue based on the current git branch    
  list                                    - List your issues                                   
  title             [issueId]             - Print the issue title                              
  start             [issueId]             - Start working on an issue                          
  view, v           [issueId]             - View issue details (default) or open in browser/app
  url               [issueId]             - Print the issue URL                                
  describe          [issueId]             - Print the issue title and Linear-issue trailer     
  commits           [issueId]             - Show all commits for a Linear issue (jj only)      
  pull-request, pr  [issueId]             - Create a GitHub pull request with issue details    
  delete, d         [issueId]             - Delete an issue                                    
  create                                  - Create a linear issue                              
  update            [issueId]             - Update a linear issue                              
  comment                                 - Manage issue comments                              
  attach            <issueId> <filepath>  - Attach a file to an issue                          
  relation                                - Manage issue relations (dependencies)
```

## Subcommands

### id

> Print the issue based on the current git branch

```
Usage:   linear issue id
Version: 1.10.0         

Description:

  Print the issue based on the current git branch

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### list

> List your issues

```
Usage:   linear issue list
Version: 1.10.0           

Description:

  List your issues

Options:

  -h, --help                       - Show this help.                                                                                                              
  -w, --workspace      <slug>      - Target workspace (uses credentials)                                                                                          
  -s, --state          <state>     - Filter by issue state (can be repeated for multiple states)           (Default: [ "unstarted" ], Values: "triage", "backlog",
                                                                                                           "unstarted", "started", "completed", "canceled")       
  --all-states                     - Show issues from all states                                                                                                  
  --assignee           <assignee>  - Filter by assignee (username)                                                                                                
  -A, --all-assignees              - Show issues for all assignees                                                                                                
  -U, --unassigned                 - Show only unassigned issues                                                                                                  
  --sort               <sort>      - Sort order (can also be set via LINEAR_ISSUE_SORT)                    (Values: "manual", "priority")                         
  --team               <team>      - Team to list issues for (if not your default team)                                                                           
  --project            <project>   - Filter by project name                                                                                                       
  --limit              <limit>     - Maximum number of issues to fetch (default: 50, use 0 for unlimited)  (Default: 50)                                          
  -w, --web                        - Open in web browser                                                                                                          
  -a, --app                        - Open in Linear.app                                                                                                           
  --no-pager                       - Disable automatic paging for long output
```

### title

> Print the issue title

```
Usage:   linear issue title [issueId]
Version: 1.10.0                      

Description:

  Print the issue title

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### start

> Start working on an issue

```
Usage:   linear issue start [issueId]
Version: 1.10.0                      

Description:

  Start working on an issue

Options:

  -h, --help                      - Show this help.                                            
  -w, --workspace      <slug>     - Target workspace (uses credentials)                        
  -A, --all-assignees             - Show issues for all assignees                              
  -U, --unassigned                - Show only unassigned issues                                
  -f, --from-ref       <fromRef>  - Git ref to create new branch from                          
  -b, --branch         <branch>   - Custom branch name to use instead of the issue identifier
```

### view

> View issue details (default) or open in browser/app

```
Usage:   linear issue view [issueId]
Version: 1.10.0                     

Description:

  View issue details (default) or open in browser/app

Options:

  -h, --help               - Show this help.                                
  -w, --workspace  <slug>  - Target workspace (uses credentials)            
  -w, --web                - Open in web browser                            
  -a, --app                - Open in Linear.app                             
  --no-comments            - Exclude comments from the output               
  --no-pager               - Disable automatic paging for long output       
  -j, --json               - Output issue data as JSON                      
  --no-download            - Keep remote URLs instead of downloading files
```

### url

> Print the issue URL

```
Usage:   linear issue url [issueId]
Version: 1.10.0                    

Description:

  Print the issue URL

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### describe

> Print the issue title and Linear-issue trailer

```
Usage:   linear issue describe [issueId]
Version: 1.10.0                         

Description:

  Print the issue title and Linear-issue trailer

Options:

  -h, --help                       - Show this help.                                                
  -w, --workspace          <slug>  - Target workspace (uses credentials)                            
  -r, --references, --ref          - Use 'References' instead of 'Fixes' for the Linear issue link
```

### commits

> Show all commits for a Linear issue (jj only)

```
Usage:   linear issue commits [issueId]
Version: 1.10.0                        

Description:

  Show all commits for a Linear issue (jj only)

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

### pull-request

> Create a GitHub pull request with issue details

```
Usage:   linear issue pull-request [issueId]
Version: 1.10.0                             

Description:

  Create a GitHub pull request with issue details

Options:

  -h, --help                 - Show this help.                                                         
  -w, --workspace  <slug>    - Target workspace (uses credentials)                                     
  --base           <branch>  - The branch into which you want your code merged                         
  --draft                    - Create the pull request as a draft                                      
  -t, --title      <title>   - Optional title for the pull request (Linear issue ID will be prefixed)  
  --web                      - Open the pull request in the browser after creating it                  
  --head           <branch>  - The branch that contains commits for your pull request
```

### delete

> Delete an issue

```
Usage:   linear issue delete [issueId]
Version: 1.10.0                       

Description:

  Delete an issue

Options:

  -h, --help                 - Show this help.                                             
  -w, --workspace  <slug>    - Target workspace (uses credentials)                         
  -y, --confirm              - Skip confirmation prompt                                    
  --bulk           <ids...>  - Delete multiple issues by identifier (e.g., TC-123 TC-124)  
  --bulk-file      <file>    - Read issue identifiers from a file (one per line)           
  --bulk-stdin               - Read issue identifiers from stdin
```

### create

> Create a linear issue

```
Usage:   linear issue create
Version: 1.10.0             

Description:

  Create a linear issue

Options:

  -h, --help                                - Show this help.                                                
  -w, --workspace            <slug>         - Target workspace (uses credentials)                            
  --start                                   - Start the issue after creation                                 
  -a, --assignee             <assignee>     - Assign the issue to 'self' or someone (by username or name)    
  --due-date                 <dueDate>      - Due date of the issue                                          
  -p, --parent               <parent>       - Parent issue (if any) as a team_number code                    
  --priority                 <priority>     - Priority of the issue (1-4, descending priority)               
  --estimate                 <estimate>     - Points estimate of the issue                                   
  -d, --description          <description>  - Description of the issue                                       
  --description-file         <path>         - Read description from a file (preferred for markdown content)  
  -l, --label                <label>        - Issue label associated with the issue. May be repeated.        
  --team                     <team>         - Team associated with the issue (if not your default team)      
  --project                  <project>      - Name of the project with the issue                             
  -s, --state                <state>        - Workflow state for the issue (by name or type)                 
  --milestone                <milestone>    - Name of the project milestone                                  
  --cycle                    <cycle>        - Cycle name, number, or 'active'                                
  --no-use-default-template                 - Do not use default template for the issue                      
  --no-interactive                          - Disable interactive prompts                                    
  -t, --title                <title>        - Title of the issue
```

### update

> Update a linear issue

```
Usage:   linear issue update [issueId]
Version: 1.10.0                       

Description:

  Update a linear issue

Options:

  -h, --help                         - Show this help.                                                
  -w, --workspace     <slug>         - Target workspace (uses credentials)                            
  -a, --assignee      <assignee>     - Assign the issue to 'self' or someone (by username or name)    
  --due-date          <dueDate>      - Due date of the issue                                          
  -p, --parent        <parent>       - Parent issue (if any) as a team_number code                    
  --priority          <priority>     - Priority of the issue (1-4, descending priority)               
  --estimate          <estimate>     - Points estimate of the issue                                   
  -d, --description   <description>  - Description of the issue                                       
  --description-file  <path>         - Read description from a file (preferred for markdown content)  
  -l, --label         <label>        - Issue label associated with the issue. May be repeated.        
  --team              <team>         - Team associated with the issue (if not your default team)      
  --project           <project>      - Name of the project with the issue                             
  -s, --state         <state>        - Workflow state for the issue (by name or type)                 
  --milestone         <milestone>    - Name of the project milestone                                  
  --cycle             <cycle>        - Cycle name, number, or 'active'                                
  -t, --title         <title>        - Title of the issue
```

### comment

> Manage issue comments

```
Usage:   linear issue comment
Version: 1.10.0              

Description:

  Manage issue comments

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  add     [issueId]    - Add a comment to an issue or reply to a comment
  update  <commentId>  - Update an existing comment                     
  list    [issueId]    - List comments for an issue
```

#### comment subcommands

##### add

```
Usage:   linear issue comment add [issueId]
Version: 1.10.0                            

Description:

  Add a comment to an issue or reply to a comment

Options:

  -h, --help                   - Show this help.                                                 
  -w, --workspace  <slug>      - Target workspace (uses credentials)                             
  -b, --body       <text>      - Comment body text                                               
  --body-file      <path>      - Read comment body from a file (preferred for markdown content)  
  -p, --parent     <id>        - Parent comment ID for replies                                   
  -a, --attach     <filepath>  - Attach a file to the comment (can be used multiple times)
```

##### update

```
Usage:   linear issue comment update <commentId>
Version: 1.10.0                                 

Description:

  Update an existing comment

Options:

  -h, --help               - Show this help.                                                 
  -w, --workspace  <slug>  - Target workspace (uses credentials)                             
  -b, --body       <text>  - New comment body text                                           
  --body-file      <path>  - Read comment body from a file (preferred for markdown content)
```

##### list

```
Usage:   linear issue comment list [issueId]
Version: 1.10.0                             

Description:

  List comments for an issue

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  
  -j, --json               - Output as JSON
```

### attach

> Attach a file to an issue

```
Usage:   linear issue attach <issueId> <filepath>
Version: 1.10.0                                  

Description:

  Attach a file to an issue

Options:

  -h, --help                - Show this help.                              
  -w, --workspace  <slug>   - Target workspace (uses credentials)          
  -t, --title      <title>  - Custom title for the attachment              
  -c, --comment    <body>   - Add a comment body linked to the attachment
```

### relation

> Manage issue relations (dependencies)

```
Usage:   linear issue relation
Version: 1.10.0               

Description:

  Manage issue relations (dependencies)

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  add     <issueId> <relationType> <relatedIssueId>  - Add a relation between two issues   
  delete  <issueId> <relationType> <relatedIssueId>  - Delete a relation between two issues
  list    [issueId]                                  - List relations for an issue
```

#### relation subcommands

##### add

```
Usage:   linear issue relation add <issueId> <relationType> <relatedIssueId>
Version: 1.10.0                                                             

Description:

  Add a relation between two issues

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Examples:

  Mark issue as blocked by another linear issue relation add ENG-123 blocked-by ENG-100
  Mark issue as blocking another   linear issue relation add ENG-123 blocks ENG-456    
  Mark issues as related           linear issue relation add ENG-123 related ENG-456   
  Mark issue as duplicate          linear issue relation add ENG-123 duplicate ENG-100
```

##### delete

```
Usage:   linear issue relation delete <issueId> <relationType> <relatedIssueId>
Version: 1.10.0                                                                

Description:

  Delete a relation between two issues

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```

##### list

```
Usage:   linear issue relation list [issueId]
Version: 1.10.0                              

Description:

  List relations for an issue

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)
```
