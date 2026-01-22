# Organization Features

Detailed command reference for Linear CLI organization features: initiatives, labels, projects, and bulk operations.

## Initiative Management

```bash
# List initiatives (default: active only)
linear initiative list
linear initiative list --all-statuses
linear initiative list --status planned

# View initiative details
linear initiative view <id-or-slug>

# Create initiative
linear initiative create --name "Q1 Goals" --status active
linear initiative create -i  # Interactive mode

# Archive/unarchive
linear initiative archive <id>
linear initiative unarchive <id>

# Link projects to initiatives
linear initiative add-project <initiative> <project>
linear initiative remove-project <initiative> <project>
```

## Label Management

```bash
# List labels (shows ID, name, color, team)
linear label list
linear label list --team DEV
linear label list --workspace  # Workspace-level only

# Create label
linear label create --name "Bug" --color "#EB5757"
linear label create --name "Feature" --team DEV

# Delete label (by ID or name)
linear label delete <id>
linear label delete "Bug" --team DEV
```

## Project Management

```bash
# List projects
linear project list

# View project
linear project view <id>

# Create project
linear project create --name "New Feature" --team DEV
linear project create --name "Q1 Work" --team DEV --initiative "Q1 Goals"
linear project create -i  # Interactive mode
```

## Bulk Operations

```bash
# Delete multiple issues
linear issue delete --bulk DEV-123 DEV-124 DEV-125

# Delete from file (one ID per line)
linear issue delete --bulk-file issues.txt

# Delete from stdin
echo -e "DEV-123\nDEV-124" | linear issue delete --bulk-stdin

# Archive multiple initiatives
linear initiative archive --bulk <id1> <id2>
```

## Adding Labels to Issues

```bash
# Add label to issue
linear issue update DEV-123 --label "Bug"

# Add multiple labels
linear issue update DEV-123 --label "Bug" --label "High Priority"
```
