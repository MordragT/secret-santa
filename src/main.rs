#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

use rand::seq::SliceRandom;
use rocket::request::{Form, FormItems, FromForm};
use rocket::response::Redirect;
use rocket::State;
use rocket_contrib::json::Json;
use rocket_contrib::serve::StaticFiles;
use rocket_contrib::templates::Template;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::RwLock;
//use std::cmp::{Eq, PartialEq};
use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub enum DraftError {
    InvalidData,
    MemberAlreadyDefined,
    NotEnoughPossibilities,
    NoTeamOrNameDefined,
    CalculateAgain,
}

impl std::error::Error for DraftError {}

impl fmt::Display for DraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DraftError::InvalidData => f.write_str("Invalid form data"),
            DraftError::MemberAlreadyDefined => f.write_str("Member was already defined"),
            DraftError::NotEnoughPossibilities => f.write_str("Not enough possibilites"),
            DraftError::NoTeamOrNameDefined => f.write_str("No team or name defined"),
            DraftError::CalculateAgain => {
                f.write_str("Took wrong path on caluclation, calculate again")
            }
        }
    }
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
struct Member {
    name: String,
    team: u32,
    ticket: Option<String>,
}

impl Member {
    fn new(name: String, team: u32) -> Member {
        Member {
            name,
            team,
            ticket: None,
        }
    }
}

impl Hash for Member {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Draft {
    title: String,
    date: String,
    members: HashSet<Member>,
}

impl fmt::Display for Draft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let members = self
            .members
            .iter()
            .map(|member| match &member.ticket {
                Some(ticket) => format!("{} -> {}", member.name, ticket),
                None => format!("{}", member.name),
            })
            .collect::<Vec<String>>();
        f.write_str(&format!("{}\n{}\n{:?}", self.title, self.date, members))
    }
}

impl Draft {
    fn team_possibilities(&self, team: u32) -> u32 {
        self.members
            .iter()
            .fold(0, |x, member| if member.team != team { x + 1 } else { x })
    }
    fn team_len(&self, team: u32) -> u32 {
        self.members
            .iter()
            .fold(0, |x, member| if member.team == team { x + 1 } else { x })
    }
    fn find_ticket(&self, member: &Member, used: &Vec<&Member>) -> Option<&Member> {
        let entries = self
            .members
            .iter()
            .filter(|other| {
                if *member != **other && member.team != other.team && !used.contains(other) {
                    true
                } else {
                    false
                }
            })
            .collect::<Vec<&Member>>();
        match entries.choose(&mut rand::thread_rng()) {
            Some(e) => Some(*e),
            None => None,
        }
    }
    fn calculate_tickets(&mut self) -> Result<(), DraftError> {
        let filtered_members = self.members.iter().find(|member| {
            if self.team_possibilities(member.team) < self.team_len(member.team) {
                true
            } else {
                false
            }
        });
        if let Some(_) = filtered_members {
            return Err(DraftError::NotEnoughPossibilities);
        }
        let mut used = Vec::new();
        let calulated_members = self
            .members
            .iter()
            .map(|member| {
                let ticket = match self.find_ticket(member, &used) {
                    Some(ticket) => ticket,
                    None => return Err(DraftError::CalculateAgain),
                };
                used.push(ticket);
                let mut new_member = member.clone();
                new_member.ticket = Some(ticket.name.clone());
                Ok(new_member)
            })
            .collect::<Result<HashSet<Member>, _>>();
        match calulated_members {
            Ok(members) => {
                self.members = members;
                Ok(())
            }
            Err(DraftError::CalculateAgain) => self.calculate_tickets(),
            Err(e) => Err(e),
        }
    }
}

impl<'f> FromForm<'f> for Draft {
    type Error = DraftError;

    fn from_form(items: &mut FormItems<'f>, _strict: bool) -> Result<Self, Self::Error> {
        let mut draft = Draft {
            title: String::new(),
            date: String::new(),
            members: HashSet::new(),
        };
        let mut name = None;
        for item in items {
            let key: &str = &*item.key;
            let value = item.value.to_string();
            if value == "" {
                return Err(Self::Error::InvalidData);
            }
            //println!("{}", value);
            match key {
                "title" => draft.title = value,
                "date" => draft.date = value,
                "name" => name = Some(value),
                "team" => match name {
                    Some(n) => {
                        draft
                            .members
                            .insert(Member::new(n, u32::from_str_radix(&value, 10).unwrap()));
                        name = None;
                    }
                    None => {
                        return Err(Self::Error::InvalidData);
                    }
                },
                e => {
                    println!("Error: Could not parse {}", e);
                    return Err(Self::Error::InvalidData);
                }
            }
        }
        draft.calculate_tickets()?;
        println!("{}", draft);
        Ok(draft)
    }
}

type Drafts = RwLock<Vec<Draft>>;

#[get("/api/draft")]
fn api_drafts(drafts: State<Drafts>) -> Json<Option<Vec<Draft>>> {
    match drafts.read() {
        Ok(drafts) => Json(Some(drafts.to_vec())),
        Err(_) => Json(None),
    }
}

#[post("/api/draft", data = "<draft_form>")]
fn api_post_draft(draft_form: Form<Draft>, drafts: State<Drafts>) -> Json<Option<usize>> {
    match drafts.write() {
        Ok(mut drafts) => {
            let draft = draft_form.into_inner();
            drafts.push(draft);
            Json(Some(drafts.len() - 1))
        }
        Err(_) => Json(None),
    }
}

#[get("/api/draft/<draft>")]
fn api_draft(draft: usize, drafts: State<Drafts>) -> Json<Option<Draft>> {
    match drafts.read() {
        Ok(drafts) => match drafts.get(draft) {
            Some(draft) => Json(Some(draft.clone())),
            None => Json(None),
        },
        Err(_) => Json(None),
    }
}

// #[get("/api/draft/<draft>/ticket")]
// fn api_draft_tickets(draft: usize, drafts: State<Drafts>) -> Json<Option<HashMap<String, String>>> {
//     match drafts.read() {
//         Ok(drafts) => match drafts.get(draft) {
//             Some(draft) => Json(Some(draft.t)),
//             None => Json(None),
//         },
//         Err(_) => Json(None),
//     }
// }

// #[post("/api/draft/<draft>/ticket", data = "<ticket_value>")]
// fn api_post_draft_ticket(draft: usize, ticket_value: String, drafts: State<Drafts>) -> Json<bool> {
//     match drafts.write() {
//         Ok(mut drafts) => match drafts.get_mut(draft) {
//             Some(draft) => Json(draft.tickets.insert(ticket_value)),
//             None => Json(false),
//         },
//         Err(_) => Json(false),
//     }
// }

#[get("/api/draft/<draft>/ticket/<name>")]
fn api_draft_ticket(draft: usize, name: String, drafts: State<Drafts>) -> Json<Option<String>> {
    match drafts.read() {
        Ok(drafts) => match drafts.get(draft) {
            Some(draft) => match draft.members.iter().find(|member| member.name == name) {
                Some(member) => Json(member.ticket.clone()),
                None => Json(None),
            },
            None => Json(None),
        },
        Err(_) => Json(None),
    }
}

#[get("/error/500")]
fn show_internal_error() -> Template {
    let context: HashMap<&str, &str> = HashMap::new();
    Template::render("500", context)
}

#[get("/")]
fn show_index(drafts: State<Drafts>) -> Template {
    let mut context = HashMap::new();
    context.insert("drafts", drafts.read().unwrap().to_vec());
    Template::render("index", context)
}

#[get("/draft")]
fn show_insert_draft() -> Template {
    let context: HashMap<&str, &str> = HashMap::new();
    Template::render("draft_insertion", context)
}

#[post("/draft", data = "<draft>")]
fn insert_draft(draft: Form<Draft>, drafts: State<Drafts>) -> Redirect {
    match api_post_draft(draft, drafts).0 {
        Some(id) => Redirect::to(uri!(show_draft: id)),
        None => Redirect::to(uri!(show_internal_error)),
    }
}

#[get("/draft/<id>")]
fn show_draft(id: usize, drafts: State<Drafts>) -> Template {
    let mut context = HashMap::new();
    match api_draft(id, drafts).0 {
        Some(draft) => {
            context.insert("draft", draft);
            Template::render("draft", context)
        }
        None => Template::render("draft_not_found", context),
    }
}

#[get("/draft/<id>/ticket/<name>")]
fn show_ticket(id: usize, name: String, drafts: State<Drafts>) -> Template {
    let mut context = HashMap::new();
    context.insert("id", id.to_string());
    match api_draft_ticket(id, name.clone(), drafts).0 {
        Some(ticket) => {
            context.insert("name", name);
            context.insert("ticket", ticket);
            Template::render("ticket", context)
        }
        None => Template::render("ticket_not_found", context),
    }
}

// #[post("/draft/<id>/ticket", data = "<name>")]
// fn insert_ticket(id: usize, name: String, drafts: State<Drafts>) -> Redirect {
//     match api_post_draft_ticket(id, name, drafts).0 {
//         true => Redirect::to(uri!(show_draft: id)),
//         false => Redirect::to(uri!(show_internal_error)),
//     }
// }

// #[get("/draft/<id>/retry/<old_ticket>")]
// fn retry_ticket(id: usize, old_ticket: String, drafts: State<Drafts>) -> Redirect {
//     match api_post_draft_ticket(id, old_ticket, drafts).0 {
//         true => Redirect::to(uri!(show_draft: id)),
//         false => Redirect::to(uri!(show_internal_error)),
//     }
// }

fn main() {
    rocket::ignite()
        .mount(
            "/",
            routes![
                api_drafts,
                api_post_draft,
                api_draft,
                // api_draft_tickets,
                // api_post_draft_ticket,
                api_draft_ticket,
                show_internal_error,
                show_index,
                show_insert_draft,
                insert_draft,
                show_draft,
                show_ticket,
                // insert_ticket,
                // retry_ticket,
            ],
        )
        .attach(Template::fairing())
        .manage(Drafts::new(Vec::new()))
        .mount("/img", StaticFiles::from("img"))
        .mount("/css", StaticFiles::from("css"))
        .launch();
}
