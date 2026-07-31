// Structural shape shared by team members and workspace members. Both generated
// node types satisfy this, so the renderer stays decoupled from codegen symbols.
export interface MemberDisplayFields {
  name: string
  displayName: string
  email: string
  active: boolean
  initials: string
  description?: string | null
  timezone?: string | null
  lastSeen?: string | null
  statusEmoji?: string | null
  statusLabel?: string | null
  guest: boolean
  isAssignable: boolean
  admin: boolean
  owner: boolean
  isMe: boolean
}

// admin and owner are independent in Linear's schema — an owner who is also an
// admin shows both markers.
function markersFor(member: MemberDisplayFields): string {
  const markers = []
  if (!member.active) markers.push("inactive")
  if (member.guest) markers.push("guest")
  if (!member.isAssignable) markers.push("not assignable")
  if (member.admin) markers.push("admin")
  if (member.owner) markers.push("owner")
  if (member.isMe) markers.push("you")

  return markers.map((marker) => ` (${marker})`).join("")
}

export function printMembers(
  members: readonly MemberDisplayFields[],
  heading: string,
): void {
  console.log(`${heading} (${members.length}):`)
  console.log("")

  for (const member of members) {
    const displayName = member.displayName || member.name
    const fullName = member.name !== member.displayName
      ? ` (${member.name})`
      : ""

    console.log(
      `${displayName}${fullName} [${member.initials}]${markersFor(member)}`,
    )
    if (member.email) {
      console.log(`  Email: ${member.email}`)
    }
    if (member.description) {
      console.log(`  Role: ${member.description}`)
    }
    if (member.timezone) {
      console.log(`  Timezone: ${member.timezone}`)
    }
    if (member.statusEmoji && member.statusLabel) {
      console.log(`  Status: ${member.statusEmoji} ${member.statusLabel}`)
    }
    if (member.lastSeen) {
      const lastSeenDate = new Date(member.lastSeen)
      console.log(`  Last seen: ${lastSeenDate.toLocaleString()}`)
    }
    console.log("")
  }
}
