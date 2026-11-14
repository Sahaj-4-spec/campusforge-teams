# 🚀 CampusForge Teams

### A Smart Real-Time Project Management Platform for Student Teams

<p align="center">
  <strong>Plan Better. Collaborate Smarter. Build Together.</strong>
</p>

CampusForge Teams is a collaborative project management platform designed to help college students and development teams organize projects, manage tasks, collaborate with teammates, and track their progress in real time.

The platform provides a centralized workspace where team members can create projects, invite teammates, assign responsibilities, manage deadlines, exchange comments, and monitor project activities.

This project is being developed as part of the **ALTA BUILDER 30 Challenge — Track A (Full-Stack Product)**.

---

## 🎯 Problem Statement

Student teams often manage their projects through disconnected messaging applications, spreadsheets, and informal discussions.

This can lead to:

- Unclear task responsibilities.
- Missed project deadlines.
- Poor communication between team members.
- Difficulty tracking project progress.
- Lack of visibility into individual contributions.

CampusForge Teams aims to solve these problems through one organized and collaborative workspace.

## 💡 Proposed Solution

CampusForge Teams provides a Trello-inspired Kanban workspace with real-time collaboration capabilities.

Users will be able to create project boards, invite teammates, organize tasks into lists, assign responsibilities, and monitor progress without constantly refreshing the application.

## ✨ Planned Features

### 1. Authentication and User Profiles
- User registration and login.
- Secure password hashing.
- Protected application routes.
- User profiles with names and avatars.

### 2. Project and Board Management
- Create and manage project boards.
- Invite team members.
- Join existing boards.
- Manage board membership.

### 3. Task Management
- Create, edit, and delete tasks.
- Add descriptions and due dates.
- Assign tasks to team members.
- Organize tasks into different lists.

### 4. Comments and Activity History
- Add comments to tasks.
- View project activity.
- Track important changes made by team members.

### 5. Advanced Feature: Real-Time Drag and Drop
- Drag tasks between lists.
- Reorder cards within lists.
- Save card positions in MongoDB.
- Synchronize board changes with other connected members using Socket.IO.

### 6. Additional Feature: Deadline Risk Radar
- Identify overdue tasks.
- Highlight upcoming deadlines.
- Display project completion statistics.
- Identify tasks without assigned members.

## 🛠️ Technology Stack

| Technology | Purpose |
|---|---|
| React + Vite | Frontend development |
| Tailwind CSS | Responsive user interface |
| Node.js | Backend runtime |
| Express.js | REST API development |
| MongoDB | Database |
| Mongoose | Database modelling |
| Socket.IO | Real-time communication |
| JWT / Secure Cookies | Authentication |
| Git and GitHub | Version control |

## 🏗️ Planned Architecture

```text
              CAMPUSFORGE TEAMS
                      |
          +-----------+-----------+
          |                       |
      FRONTEND                 BACKEND
          |                       |
    React + Vite             Node.js
          |                       |
     Tailwind CSS              Express
          |                       |
          +---------- API --------+
                      |
               +------+------+
               |             |
            MongoDB       Socket.IO
               |             |
          Data Storage   Real-Time Events
```

## 📁 Planned Project Structure

```text
campusforge-teams/
│
├── client/
│   ├── public/
│   └── src/
│       ├── components/
│       ├── pages/
│       ├── hooks/
│       ├── services/
│       ├── context/
│       └── App.jsx
│
├── server/
│   ├── config/
│   ├── controllers/
│   ├── middleware/
│   ├── models/
│   ├── routes/
│   ├── sockets/
│   └── server.js
│
├── .gitignore
├── README.md
└── package.json
```

## 📅 Development Roadmap

This project follows the 15-day Track A development schedule of ALTA BUILDER 30.

| Days | Development Milestone |
|---|---|
| 1–2 | Project setup and database connection |
| 3–4 | Authentication and board management |
| 5–6 | Lists, cards and task assignments |
| 7–8 | Comments, activities and backend validation |
| 9–10 | Dashboard, integration and MVP |
| 11–12 | Drag and drop and real-time synchronization |
| 13–14 | Testing, deployment and documentation |
| 15 | Final project demonstration and launch |

## 🔒 Security Considerations

- Passwords will be stored using secure hashing.
- Private API endpoints will require authentication.
- Board access will be restricted to authorized members.
- Backend inputs will be validated.
- Environment variables will be used for sensitive configuration.
- API keys and database credentials will never be committed to GitHub.

## 📖 What I Built and What I Understood

This section will be updated throughout development.

I aim to understand and implement:

1. How React communicates with a backend through REST APIs.
2. How Node.js and Express handle application requests.
3. How MongoDB stores users, projects, lists, and tasks.
4. How authentication and authorization protect application data.
5. How drag-and-drop interactions update persistent database records.
6. How Socket.IO enables real-time communication between multiple users.
7. How to build and deploy a responsive full-stack application.

## 🤖 AI Tools and Development Assistance

AI-assisted development tools may be used for:
- Understanding technical concepts.
- Exploring implementation approaches.
- Debugging and troubleshooting.
- Improving code quality and documentation.

All AI-assisted code will be reviewed, tested, and understood before being incorporated into the project.

## 🚧 Current Project Status

**Status:** Initial development — Day 1 of ALTA BUILDER 30.

Features listed above represent the development plan. Implementation status will be updated as functionality is completed and tested.

## 🎓 Challenge Information

**Challenge:** ALTA BUILDER 30  
**Track:** A — Full-Stack Product  
**Project:** Project Board (Trello-style)  
**Duration:** 15 Days

## 👨‍💻 Developer

Developed as an independent project for the ALTA BUILDER 30 Challenge.

---

<p align="center">
  <strong>CampusForge Teams — Build Together. Achieve Together.</strong>
</p>

<p align="center">
  ⭐ Star this repository if you find the project interesting!
</p>
