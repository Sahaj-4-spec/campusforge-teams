import { useMemo, useState } from "react";
import {
  ArrowRight, BarChart3, BookOpen, CalendarDays, CheckCircle2, ChevronDown,
  CircleHelp, FileText, FolderKanban, ListTodo, MessageCircle, Search,
  Settings, ShieldCheck, Users, Bell, Activity
} from "lucide-react";

const guideSections = [
  {
    id: "projects",
    title: "Projects and task boards",
    summary: "Create a project, add work, and keep tasks moving through board columns.",
    icon: FolderKanban,
    view: "Projects",
    steps: [
      "Open Projects from the left navigation. Select New project if you have permission to create one, then enter the project details and save.",
      "Open the project card to see its board. Use New task or Add task in a column to create work.",
      "Give each task a clear title. Add a description, status list, priority, due date, assignee, and estimated time where useful, then save it.",
      "Open a task card to review or update its details and add a comment for the team.",
      "Drag a task card to another column when its status changes. Project admins can also add, rename, or remove empty columns."
    ],
    tip: "Keep task titles action-oriented, assign one clear owner, and add a due date when work has a deadline."
  },
  {
    id: "my-tasks",
    title: "Use My Tasks to manage your own work",
    summary: "See the tasks assigned to you across your projects.",
    icon: ListTodo,
    view: "My Tasks",
    steps: [
      "Choose My tasks from the left navigation.",
      "Use the filters to view all your assigned tasks, high-priority work, tasks in progress, or completed tasks.",
      "Use the search box in the top bar to find a task by words in its title or description.",
      "Select a task row to open it, review the details, and update its status or other fields."
    ],
    tip: "My Tasks shows work assigned to your account. If a task is missing, open it from its project board and check the assignee."
  },
  {
    id: "team",
    title: "Invite and work with teammates",
    summary: "Add people to the workspace and give them the right level of access.",
    icon: Users,
    view: "Team",
    steps: [
      "Open Team from the left navigation to see workspace members.",
      "Workspace owners and admins can use Invite a teammate, enter the teammate's email address, choose a role, and create an invitation link.",
      "Copy the invitation link and send it to the person using the email address entered for the invitation.",
      "The invitation is single-use and expires after seven days. The invited person must accept it using an account with the invited email address.",
      "Members who are not owners or admins can view the team list but cannot create invitations."
    ],
    tip: "Only invite people who should have access to this workspace. Check the selected role before sharing the invitation link."
  },
  {
    id: "calendar",
    title: "Plan deadlines and events in Calendar",
    summary: "Bring task due dates, meetings, milestones, exams, and submissions into one view.",
    icon: CalendarDays,
    view: "Calendar",
    steps: [
      "Open Calendar from the left navigation.",
      "Choose Month, Week, or Day to change the amount of detail shown. Use the arrows to move between periods or Today to return to the current date.",
      "Select Add event to create a meeting, project milestone, exam, submission, or focus block.",
      "Enter a title, choose an event type, optionally link the event to a project, and add a description.",
      "Set the start and end time, or choose All-day event and select a date. Save the event when finished.",
      "Task deadlines appear in the calendar alongside events. Select a task to open its details."
    ],
    tip: "Add milestones and submission dates early, then use the weekly view to plan the team's near-term work."
  },
  {
    id: "notifications",
    title: "Manage notifications",
    summary: "Keep up with assignments, comments, project updates, and approaching deadlines.",
    icon: Bell,
    view: "Notifications",
    steps: [
      "Open Notifications from the left navigation, or select the bell in the top bar.",
      "Unread items are highlighted. Select the check icon on an item to mark it as read, or use Mark all as read.",
      "Expand Notification preferences to choose which categories appear in the in-app notification centre.",
      "Where email delivery has been enabled for the workspace, you can also manage email preferences by category."
    ],
    tip: "Review upcoming-deadline and overdue-task notifications regularly so important work does not get missed."
  },
  {
    id: "files",
    title: "Store and share files",
    summary: "Keep project documents and attachments available from the Files area.",
    icon: FileText,
    view: "Files",
    steps: [
      "Open Files & docs from the left navigation.",
      "Use the upload control to add a supported document or attachment. Follow the on-screen file-size and file-type limits.",
      "Browse the file list and use the available download action when you need a copy.",
      "Keep important originals in your own storage until the workspace's durable cloud file-storage setup is complete."
    ],
    tip: "Use descriptive file names such as Project-Brief-v2.pdf so teammates can identify the right document."
  },
  {
    id: "chat",
    title: "Use team chat and meeting notes",
    summary: "Keep conversations, decisions, agendas, and follow-up actions connected to the workspace.",
    icon: MessageCircle,
    view: "Chat",
    steps: [
      "Open Chat & notes from the left navigation.",
      "In Team chat, choose General workspace or select a project channel, write your message, and select Send.",
      "Choose Meeting notes to record a meeting title, date, project, attendees, agenda, discussion notes, and action items.",
      "Select Save meeting notes. Saved notes appear alongside the form and can be reviewed later."
    ],
    tip: "Record decisions and action owners in meeting notes so follow-up work is easy to find."
  },
  {
    id: "insights",
    title: "Review activity and analytics",
    summary: "Understand recent changes and how work is progressing.",
    icon: BarChart3,
    view: "Reports",
    steps: [
      "Open Timeline to review recent activity across the workspace.",
      "Open Analytics to review the available productivity and project summaries.",
      "Use the timeline to understand what changed and use analytics to spot workload or completion patterns.",
      "Open the related project or task when you need to follow up on an item."
    ],
    tip: "Use the same date range and team scope when comparing reports over time."
  },
  {
    id: "account",
    title: "Switch workspaces and manage your account",
    summary: "Find your workspace, update account preferences, and sign out safely.",
    icon: Settings,
    view: "Settings",
    steps: [
      "Use the workspace selector near the top of the left sidebar to switch between workspaces your account can access.",
      "Select your profile card at the bottom of the sidebar to open Account settings or Sign out.",
      "The top-right profile avatar also opens Settings.",
      "Open Settings to confirm the signed-in account and end your current session with Sign out."
    ],
    tip: "Always sign out when using a shared or public computer."
  }
];

const quickSteps = [
  { number: "01", title: "Open Projects", detail: "Create or choose a project.", view: "Projects", icon: FolderKanban },
  { number: "02", title: "Add tasks", detail: "Set owners, priorities, and dates.", view: "Projects", icon: ListTodo },
  { number: "03", title: "Invite your team", detail: "Share access with collaborators.", view: "Team", icon: Users },
  { number: "04", title: "Plan the work", detail: "Track meetings and deadlines.", view: "Calendar", icon: CalendarDays }
];

export default function HelpPage({ onNavigate }) {
  const [expanded, setExpanded] = useState("projects");
  const [search, setSearch] = useState("");

  const visibleSections = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return guideSections;
    return guideSections.filter((section) =>
      [section.title, section.summary, section.tip, ...section.steps].join(" ").toLowerCase().includes(query)
    );
  }, [search]);

  return (
    <div className="page-stack help-page">
      <div className="page-title-row">
        <div>
          <p className="eyebrow">CAMPUSFORGE USER GUIDE</p>
          <h1>Help &amp; support</h1>
          <p className="page-subtitle">Follow these steps to set up your workspace, organise projects, and collaborate with your team.</p>
        </div>
        <span className="help-guide-badge"><BookOpen size={15} /> Getting started</span>
      </div>

      <section className="help-guide-hero">
        <div className="help-guide-hero-copy">
          <span className="help-guide-kicker"><CheckCircle2 size={14} /> YOUR FIRST WORKSPACE</span>
          <h2>Get your team from setup to shared progress.</h2>
          <p>Start with a project, give everyone clear tasks, and use the shared tools to keep deadlines and conversations in one place.</p>
          <button className="primary-button" type="button" onClick={() => onNavigate("Projects")}>
            Start with Projects <ArrowRight size={15} />
          </button>
        </div>
        <div className="help-guide-hero-mark" aria-hidden="true"><BookOpen size={54} /></div>
      </section>

      <section className="help-quickstart">
        <div className="help-guide-section-heading">
          <div><p className="eyebrow">QUICK START</p><h2>Four steps to get going</h2></div>
          <span>Follow in order</span>
        </div>
        <div className="help-quickstart-grid">
          {quickSteps.map(({ number, title, detail, view, icon: Icon }) => (
            <button className="help-quickstep" type="button" key={number} onClick={() => onNavigate(view)}>
              <span className="help-quickstep-number">{number}</span>
              <span className="help-quickstep-icon"><Icon size={18} /></span>
              <strong>{title}</strong>
              <small>{detail}</small>
              <span className="help-quickstep-link">Open <ArrowRight size={13} /></span>
            </button>
          ))}
        </div>
      </section>

      <section className="help-guide-list">
        <div className="help-guide-section-heading">
          <div><p className="eyebrow">STEP-BY-STEP INSTRUCTIONS</p><h2>How to use each feature</h2></div>
          <label className="help-guide-search">
            <Search size={15} />
            <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search this guide..." aria-label="Search help guide" />
          </label>
        </div>
        {visibleSections.length ? visibleSections.map((section, index) => {
          const Icon = section.icon;
          const isOpen = expanded === section.id;
          return (
            <article className={"help-guide-card" + (isOpen ? " is-expanded" : "")} key={section.id}>
              <button
                className="help-guide-toggle"
                type="button"
                aria-expanded={isOpen}
                aria-controls={"help-guide-content-" + section.id}
                onClick={() => setExpanded(isOpen ? "" : section.id)}
              >
                <span className="help-guide-icon"><Icon size={18} /></span>
                <span className="help-guide-toggle-copy"><strong>{section.title}</strong><small>{section.summary}</small></span>
                <ChevronDown className="help-guide-chevron" size={18} aria-hidden="true" />
              </button>
              <div className="help-guide-content" id={"help-guide-content-" + section.id} hidden={!isOpen}>
                <ol>
                  {section.steps.map((step, stepIndex) => <li key={stepIndex}><span>{stepIndex + 1}</span><p>{step}</p></li>)}
                </ol>
                <div className="help-guide-tip"><ShieldCheck size={16} /><p><strong>Tip:</strong> {section.tip}</p></div>
                <div className="help-guide-actions">
                  <button className="secondary-button" type="button" onClick={() => onNavigate(section.view)}>
                    Open {section.view} <ArrowRight size={13} />
                  </button>
                </div>
              </div>
            </article>
          );
        }) : (
          <div className="help-guide-empty"><CircleHelp size={22} /><strong>No guide sections found</strong><p>Try another word, such as projects, tasks, calendar, or invites.</p></div>
        )}
      </section>

      <section className="help-support-note">
        <div className="help-support-note-icon"><CircleHelp size={20} /></div>
        <div>
          <h2>Still need help?</h2>
          <p>For workspace access or invitation problems, contact your workspace owner or admin. For a task or project question, open the relevant board and include the task title when asking your team.</p>
        </div>
        <button className="secondary-button" type="button" onClick={() => onNavigate("Team")}>View team <ArrowRight size={13} /></button>
      </section>
    </div>
  );
}
