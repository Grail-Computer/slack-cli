mod auth;
mod slack;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use slack::Slack;
use std::{fs, io::Write, path::PathBuf};

#[derive(Parser)]
#[command(
    version,
    about = "Standalone Slack CLI for macOS. Uses your Slack Desktop session; outputs JSON."
)]
struct Cli {
    /// Compatibility flag: JSON always includes raw timestamps
    #[arg(long = "ts", global = true, hide = true)]
    show_ts: bool,
    /// Compatibility flag: output is always JSON
    #[arg(long, global = true, hide = true)]
    no_emoji: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Verify the current account and workspace (never prints credentials)
    Auth {
        #[arg(long)]
        refresh: bool,
    },
    /// List public and private channels
    #[command(alias = "ch")]
    Channels,
    /// List existing direct messages
    #[command(alias = "dm")]
    Dms,
    /// List workspace users
    #[command(alias = "u")]
    Users,
    /// Read channel or DM history, newest first
    #[command(alias = "r")]
    Read {
        channel: String,
        #[arg(default_value="20",value_parser=count)]
        count: usize,
        #[arg(long)]
        threads: bool,
        /// Inclusive UTC date boundary, YYYY-MM-DD
        #[arg(long)]
        from: Option<String>,
        /// Exclusive UTC date boundary, YYYY-MM-DD
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Read a thread, oldest first (includes the parent)
    #[command(alias = "t")]
    Thread {
        channel: String,
        ts: String,
        #[arg(default_value="50",value_parser=count)]
        count: usize,
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Send a message, optionally as a thread reply
    #[command(alias = "s")]
    Send {
        channel: String,
        #[arg(required_unless_present = "text_file", conflicts_with = "text_file")]
        message: Option<String>,
        #[arg(long)]
        text_file: Option<PathBuf>,
        #[arg(long)]
        thread: Option<String>,
    },
    /// Upload a file, optionally into a thread
    Upload {
        channel: String,
        file: PathBuf,
        #[arg(conflicts_with = "caption_file")]
        caption: Option<String>,
        #[arg(long)]
        caption_file: Option<PathBuf>,
        #[arg(long)]
        thread: Option<String>,
    },
    /// Search messages using Slack search syntax
    Search {
        query: String,
        #[arg(default_value="20",value_parser=count)]
        count: usize,
        #[arg(long,default_value="1",value_parser=clap::value_parser!(u32).range(1..))]
        page: u32,
    },
    /// Add a reaction to a message
    React {
        channel: String,
        ts: String,
        emoji: String,
    },
    /// Channel and DM unread counts
    #[command(alias = "a")]
    Activity,
    /// Unread conversations, excluding muted ones
    #[command(alias = "ur")]
    Unread,
    /// Starred items and VIP users
    #[command(alias = "star")]
    Starred,
    /// Saved-for-later items (Slack's internal API)
    #[command(alias = "sv")]
    Saved {
        #[arg(default_value="20",value_parser=count)]
        count: usize,
        #[arg(long)]
        all: bool,
    },
    /// Pinned messages
    #[command(alias = "pin")]
    Pins { channel: String },
    /// Create a draft: CHANNEL TEXT | thread CHANNEL TS TEXT | user USER TEXT | drop ID
    Draft {
        #[arg(required=true,num_args=1..)]
        args: Vec<String>,
    },
    /// List active drafts
    Drafts,
    /// Call a Slack API method; params must be a JSON object. Can mutate Slack.
    Api {
        method: String,
        #[arg(long)]
        params_file: Option<PathBuf>,
    },
}
fn count(s: &str) -> std::result::Result<usize, String> {
    let n = s
        .parse::<usize>()
        .map_err(|_| "Count must be an integer".to_string())?;
    if !(1..=100_000).contains(&n) {
        return Err("Count must be 1–100000".into());
    }
    Ok(n)
}
fn text(inline: Option<String>, path: Option<PathBuf>) -> Result<String> {
    let s = match path {
        Some(p) => fs::read_to_string(p).context("Cannot read text file")?,
        None => inline.context("Message text is required")?,
    };
    if s.trim().is_empty() {
        bail!("Message text is empty");
    }
    Ok(s)
}
fn date(s: &str) -> Result<i64> {
    Ok(chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .context("Date must be YYYY-MM-DD")?
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp())
}
fn read_params(
    channel: String,
    from: Option<String>,
    to: Option<String>,
    cursor: Option<String>,
) -> Result<Value> {
    let mut p = json!({"channel":channel,"inclusive":true});
    let oldest = from.as_deref().map(date).transpose()?;
    let latest = to.as_deref().map(date).transpose()?;
    if oldest.zip(latest).is_some_and(|(a, b)| a >= b) {
        bail!("--from must be earlier than --to");
    }
    if let Some(n) = oldest {
        p["oldest"] = json!(n.to_string());
    }
    // Slack inclusive applies to both bounds; subtract one microsecond for an exclusive end.
    if let Some(n) = latest {
        p["latest"] = json!(format!("{}.999999", n - 1));
    }
    if let Some(c) = cursor {
        p["cursor"] = json!(c);
    }
    Ok(p)
}
fn draft_params(channel: String, thread: Option<String>, message: String) -> Value {
    let mut dest = json!({"channel_id":channel});
    if let Some(t) = thread {
        dest["thread_ts"] = json!(t);
        dest["broadcast"] = json!(false);
    }
    json!({"client_msg_id":uuid::Uuid::new_v4().to_string(),"is_from_composer":false,"file_ids":[],"destinations":[dest],"blocks":[{"type":"rich_text","elements":[{"type":"rich_text_section","elements":[{"type":"text","text":message}]}]}]})
}
fn run(cli: Cli) -> Result<Value> {
    // Validate local files and arguments before touching Keychain or the network.
    let prepared_text = match &cli.command {
        Command::Send {
            message, text_file, ..
        } => Some(text(message.clone(), text_file.clone())?),
        Command::Upload {
            caption,
            caption_file,
            ..
        } if caption.is_some() || caption_file.is_some() => {
            Some(text(caption.clone(), caption_file.clone())?)
        }
        _ => None,
    };
    let params = match &cli.command {
        Command::Api {
            params_file: Some(p),
            ..
        } => {
            let v: Value =
                serde_json::from_slice(&fs::read(p).context("Cannot read params file")?)?;
            if !v.is_object() {
                bail!("API params must be a JSON object");
            }
            Some(v)
        }
        Command::Read {
            channel,
            from,
            to,
            cursor,
            ..
        } => Some(read_params(
            channel.clone(),
            from.clone(),
            to.clone(),
            cursor.clone(),
        )?),
        _ => None,
    };
    let s = Slack::new(matches!(cli.command, Command::Auth { refresh: true }))?;
    match cli.command {
        Command::Auth{..}=>s.api("auth.test",json!({})),
        Command::Channels=>s.list("conversations.list",json!({"types":"public_channel,private_channel","exclude_archived":true}),"channels",100_000),
        Command::Dms=>s.list("conversations.list",json!({"types":"im,mpim","exclude_archived":true}),"channels",100_000),
        Command::Users=>s.list("users.list",json!({}),"members",100_000),
        Command::Read{channel,count,threads,..}=>{
            let ch=s.resolve(&channel,false)?;let mut p=params.unwrap();p["channel"]=json!(ch);
            let mut data=s.list("conversations.history",p,"messages",count)?;
            if threads {for msg in data["messages"].as_array_mut().context("Missing messages")?{if msg["reply_count"].as_u64().unwrap_or(0)>0 {msg["thread"]=s.list("conversations.replies",json!({"channel":ch,"ts":msg["ts"]}),"messages",100_000)?;}}}
            Ok(data)
        },
        Command::Thread{channel,ts,count,cursor}=>{let mut p=json!({"channel":s.resolve(&channel,false)?,"ts":ts});if let Some(c)=cursor{p["cursor"]=json!(c);}s.list("conversations.replies",p,"messages",count)},
        Command::Send{channel,thread,..}=>{let mut p=json!({"channel":s.resolve(&channel,true)?,"text":prepared_text.unwrap()});if let Some(t)=thread{p["thread_ts"]=json!(t);}s.api("chat.postMessage",p)},
        Command::Upload{channel,file,thread,..}=>s.upload(&s.resolve(&channel,true)?,&file,prepared_text,thread),
        Command::Search{query,count,page}=>{if count>100{bail!("Slack search supports at most 100 results per page; use --page");}s.api("search.messages",json!({"query":query,"count":count,"page":page,"sort":"timestamp","sort_dir":"desc"}))},
        Command::React{channel,ts,emoji}=>s.api("reactions.add",json!({"channel":s.resolve(&channel,false)?,"timestamp":ts,"name":emoji.trim_matches(':')})),
        Command::Pins{channel}=>s.api("pins.list",json!({"channel":s.resolve(&channel,false)?})),
        Command::Activity=>s.api("client.counts",json!({})),
        Command::Unread=>{
            let counts=s.api("client.counts",json!({}))?;let prefs=s.api("users.prefs.get",json!({}))?;
            let notifications:Value=serde_json::from_str(prefs["prefs"]["all_notifications_prefs"].as_str().unwrap_or("{}")).context("Invalid notification prefs")?;
            let muted=prefs["prefs"]["muted_channels"].as_str().unwrap_or("");
            let items:Vec<_>=["channels","ims","mpims"].iter().flat_map(|key|counts[key].as_array().into_iter().flatten()).filter(|c|{
                let id=c["id"].as_str().unwrap_or("");let mute=notifications["channels"][id]["muted"]==true||muted.split(',').any(|m|m==id);
                !mute&&(c["has_unreads"]==true||c["mention_count"].as_u64().unwrap_or(0)>0)
            }).cloned().collect();Ok(json!({"ok":true,"conversations":items}))
        },
        Command::Starred=>{let stars=s.api("stars.list",json!({"count":100}))?;let prefs=s.api("users.prefs.get",json!({}))?;Ok(json!({"ok":true,"stars":stars,"vip_users":prefs["prefs"]["vip_users"]}))},
        Command::Saved{count,all}=>{let mut data=s.api("saved.list",json!({"count":count}))?;if !all{if let Some(items)=data["saved_items"].as_array_mut(){items.retain(|i|i["state"]!="completed");}}Ok(data)},
        Command::Drafts=>{let mut d=s.api("drafts.list",json!({}))?;if let Some(ds)=d["drafts"].as_array_mut(){ds.retain(|d|d["is_deleted"]!=true&&d["is_sent"]!=true);}Ok(d)},
        Command::Draft{args}=>{
            let (channel,thread,message)=match args[0].as_str(){
                "drop"=>{if args.len()!=2{bail!("Usage: slk draft drop ID");}let ds=s.api("drafts.list",json!({}))?;let d=ds["drafts"].as_array().context("Missing drafts")?.iter().find(|d|d["id"].as_str()==Some(&args[1])).context("Draft not found")?;return s.api("drafts.delete",json!({"draft_id":args[1],"client_last_updated_ts":d["last_updated_ts"]}));},
                "thread"=>{if args.len()<4{bail!("Usage: slk draft thread CHANNEL TS TEXT");}(&args[1],Some(args[2].clone()),args[3..].join(" "))},
                "user"=>{if args.len()<3{bail!("Usage: slk draft user USER TEXT");}(&args[1],None,args[2..].join(" "))},
                _=>{if args.len()<2{bail!("Usage: slk draft CHANNEL TEXT");}(&args[0],None,args[1..].join(" "))},
            };
            let msg=text(Some(message),None)?;s.api("drafts.create",draft_params(s.resolve(channel,true)?,thread,msg))
        },
        Command::Api{method,..}=>s.api(&method,params.unwrap_or(json!({}))),
    }
}
fn main() {
    match run(Cli::parse()) {
        Ok(v) => {
            if let Err(e) = writeln!(std::io::stdout(), "{v}") {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("{}", json!({"ok":false,"error":format!("{e:#}")}));
            std::process::exit(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_ts_flag_does_not_collide_with_thread_timestamp() {
        let cli =
            Cli::try_parse_from(["slk", "thread", "C12345678", "1.0", "100", "--ts"]).unwrap();
        assert!(cli.show_ts);
        assert!(matches!(cli.command, Command::Thread { ts, count: 100, .. } if ts == "1.0"));
        assert!(Cli::try_parse_from(["slk", "react", "C12345678", "1.0", "eyes"]).is_ok());
    }
    #[test]
    fn validates_count_and_dates() {
        assert!(count("0").is_err());
        assert!(count("100001").is_err());
        assert!(date("tomorrow").is_err());
        assert!(read_params(
            "C1".into(),
            Some("2026-01-02".into()),
            Some("2026-01-01".into()),
            None
        )
        .is_err());
        let p = read_params(
            "C1".into(),
            Some("2026-01-01".into()),
            Some("2026-01-02".into()),
            None,
        )
        .unwrap();
        assert_eq!(p["oldest"], "1767225600");
        assert_eq!(p["latest"], "1767311999.999999");
    }
    #[test]
    fn multiline_and_draft_thread_payload() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("message.txt");
        fs::write(&p, "one\ntwo\n").unwrap();
        let msg = text(None, Some(p)).unwrap();
        assert_eq!(msg, "one\ntwo\n");
        let d = draft_params("C1".into(), Some("1.0".into()), msg);
        assert_eq!(d["destinations"][0]["thread_ts"], "1.0");
        assert_eq!(
            d["blocks"][0]["elements"][0]["elements"][0]["text"],
            "one\ntwo\n"
        );
    }
    #[test]
    fn cli_enforces_text_source_and_keeps_search_count_out_of_query() {
        assert!(Cli::try_parse_from(["slk", "send", "general"]).is_err());
        assert!(
            Cli::try_parse_from(["slk", "send", "general", "hello", "--text-file", "a"]).is_err()
        );
        let c = Cli::try_parse_from(["slk", "search", "build failed", "5"]).unwrap();
        assert!(matches!(c.command,Command::Search{query,count:5,..} if query=="build failed"));
    }
}
