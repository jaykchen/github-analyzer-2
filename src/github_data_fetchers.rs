use std::collections::HashSet;
use chrono::{ DateTime, Duration, Utc };
use derivative::Derivative;
use github_flows::octocrab::models::{ issues::Issue, User };
use github_flows::{ get_octo, octocrab, GithubLogin };
use serde::{ Deserialize, Serialize };

#[derive(Derivative, Serialize, Deserialize, Debug, Clone)]
pub struct GitMemory {
    pub memory_type: MemoryType,
    #[derivative(Default(value = "String::from(\"\")"))]
    pub name: String,
    #[derivative(Default(value = "String::from(\"\")"))]
    pub tag_line: String,
    #[derivative(Default(value = "String::from(\"\")"))]
    pub source_url: String,
    #[derivative(Default(value = "String::from(\"\")"))]
    pub payload: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MemoryType {
    Commit,
    Issue,
    Discussion,
    Meta,
}

pub async fn get_user_profile(user: &str) -> Option<User> {
    let user_profile_url = format!("users/{user}");

    let octocrab = get_octo(&GithubLogin::Default);

    octocrab.get::<User, _, ()>(&user_profile_url, None::<&()>).await.ok()
}

pub async fn get_user_data_by_login(login: &str) -> anyhow::Result<String> {
    #[derive(Debug, Deserialize)]
    struct User {
        name: Option<String>,
        login: Option<String>,
        url: Option<String>,
        #[serde(rename = "twitterUsername")]
        twitter_username: Option<String>,
        bio: Option<String>,
        company: Option<String>,
        location: Option<String>,
        #[serde(rename = "createdAt")]
        created_at: Option<DateTime<Utc>>,
        email: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct RepositoryOwner {
        #[serde(rename = "repositoryOwner")]
        repository_owner: Option<User>,
    }

    #[derive(Debug, Deserialize)]
    struct UserRoot {
        data: Option<RepositoryOwner>,
    }

    let mut out = String::from("USER_profile: \n");

    let query = format!(
        r#"
        query {{
            repositoryOwner(login: "{login}") {{
                ... on User {{
                    name
                    login
                    url
                    twitterUsername
                    bio
                    company
                    location
                    createdAt
                    email
                }}
            }}
        }}
        "#
    );

    let octocrab = get_octo(&GithubLogin::Default);

    let res: UserRoot = octocrab.graphql::<UserRoot>(&query).await?;
    if let Some(repository_owner) = &res.data {
        if let Some(user) = &repository_owner.repository_owner {
            let login_str = match &user.login {
                Some(login) => format!("Login: {},", login),
                None => String::new(),
            };

            let name_str = match &user.name {
                Some(name) => format!("Name: {},", name),
                None => String::new(),
            };

            let url_str = match &user.url {
                Some(url) => format!("Url: {},", url),
                None => String::new(),
            };

            let twitter_str = match &user.twitter_username {
                Some(twitter) => format!("Twitter: {},", twitter),
                None => String::new(),
            };

            let bio_str = match &user.bio {
                Some(bio) if bio.is_empty() => String::new(),
                Some(bio) => format!("Bio: {},", bio),
                None => String::new(),
            };

            let company_str = match &user.company {
                Some(company) => format!("Company: {},", company),
                None => String::new(),
            };

            let location_str = match &user.location {
                Some(location) => format!("Location: {},", location),
                None => String::new(),
            };

            let date_str = match &user.created_at {
                Some(date) => { format!("Created At: {},", date.date_naive().to_string()) }
                None => String::new(),
            };

            let email_str = match &user.email {
                Some(email) => format!("Email: {}", email),
                None => String::new(),
            };

            out.push_str(
                &format!(
                    "{name_str} {login_str} {url_str} {twitter_str} {bio_str} {company_str} {location_str} {date_str} {email_str}\n"
                )
            );
        }
    }

    Ok(out)
}

pub async fn get_community_profile_data(owner: &str, repo: &str) -> Option<String> {
    #[derive(Deserialize, Debug)]
    struct CommunityProfile {
        description: String,
        // documentation: Option<String>,
    }

    let community_profile_url = format!("repos/{owner}/{repo}/community/profile");

    let octocrab = get_octo(&GithubLogin::Default);

    match octocrab.get::<CommunityProfile, _, ()>(&community_profile_url, None::<&()>).await {
        Ok(profile) => {
            return Some(format!("Description: {}", profile.description));
        }
        Err(e) => log::error!("Error parsing Community Profile: {:?}", e),
    }
    None
}

pub async fn get_contributors(owner: &str, repo: &str) -> Result<Vec<String>, octocrab::Error> {
    #[derive(Debug, Deserialize)]
    struct GithubUser {
        login: String,
    }
    let mut contributors = Vec::new();
    let octocrab = get_octo(&GithubLogin::Default);
    'outer: for n in 1..50 {
        // log::info!("contributors loop {}", n);

        let contributors_route = format!("repos/{owner}/{repo}/contributors?per_page=100&page={n}");

        match octocrab.get::<Vec<GithubUser>, _, ()>(&contributors_route, None::<&()>).await {
            Ok(user_vec) => {
                for user in &user_vec {
                    contributors.push(user.login.clone());
                    // log::info!("user: {}", user.login);
                    // upload_airtable(&user.login, "email", "twitter_username", false).await;
                }
                if user_vec.len() < 100 {
                    break 'outer;
                }
            }

            Err(_e) => {
                log::error!("looping stopped: {:?}", _e);
                break 'outer;
            }
        }
    }

    Ok(contributors)
}

pub async fn get_recent_committers(
    owner: &str,
    repo: &str,
    n_days: u16
) -> Result<HashSet<String>, octocrab::Error> {
    let mut contributors = HashSet::new();
    match get_commits_in_range_search(owner, repo, None, n_days, None).await {
        Some((_, commits_vec)) => {
            commits_vec.into_iter().for_each(|commit| {
                contributors.insert(commit.name.clone());
            });
        }
        None => log::error!("failed to get commits"),
    }
log::info!("contributors: {:?}", contributors);
    Ok(contributors)
}

pub async fn get_readme(owner: &str, repo: &str) -> Option<String> {
    #[derive(Deserialize, Debug)]
    struct GithubReadme {
        content: Option<String>,
    }

    let readme_url = format!("repos/{owner}/{repo}/readme");

    let octocrab = get_octo(&GithubLogin::Default);

    match octocrab.get::<GithubReadme, _, ()>(&readme_url, None::<&()>).await {
        Ok(readme) => {
            if let Some(c) = readme.content {
                let cleaned_content = c.replace("\n", "");
                match base64::decode(&cleaned_content) {
                    Ok(decoded_content) =>
                        match String::from_utf8(decoded_content) {
                            Ok(out) => {
                                return Some(format!("Readme: {}", out));
                            }
                            Err(e) => {
                                log::error!("Failed to convert cleaned readme to String: {:?}", e);
                                return None;
                            }
                        }
                    Err(e) => {
                        log::error!("Error decoding base64 content: {:?}", e);
                        None
                    }
                }
            } else {
                log::error!("Content field in readme is null.");
                None
            }
        }
        Err(e) => {
            log::error!("Error parsing Readme: {:?}", e);
            None
        }
    }
}
pub async fn get_readme_owner_repo(about_repo: &str) -> Option<String> {
    #[derive(Deserialize, Debug)]
    struct GithubReadme {
        content: Option<String>,
    }

    let readme_url = format!("repos/{about_repo}/readme");

    let octocrab = get_octo(&GithubLogin::Default);

    match octocrab.get::<GithubReadme, _, ()>(&readme_url, None::<&()>).await {
        Ok(readme) => {
            if let Some(c) = readme.content {
                let cleaned_content = c.replace("\n", "");
                match base64::decode(&cleaned_content) {
                    Ok(decoded_content) =>
                        match String::from_utf8(decoded_content) {
                            Ok(out) => {
                                return Some(format!("Readme: {}", out));
                            }
                            Err(e) => {
                                log::error!("Failed to convert cleaned readme to String: {:?}", e);
                                return None;
                            }
                        }
                    Err(e) => {
                        log::error!("Error decoding base64 content: {:?}", e);
                        None
                    }
                }
            } else {
                log::error!("Content field in readme is null.");
                None
            }
        }
        Err(e) => {
            log::error!("Error parsing Readme: {:?}", e);
            None
        }
    }
}

pub async fn get_issues_in_range(
    owner: &str,
    repo: &str,
    user_name: Option<String>,
    range: u16,
    token: Option<String>
) -> Option<(usize, Vec<Issue>)> {
    #[derive(Debug, Deserialize)]
    struct Page<T> {
        pub items: Vec<T>,
        pub total_count: Option<u64>,
    }

    let n_days_ago = (Utc::now() - Duration::days(range as i64))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let user_str = user_name.map_or(String::new(), |u| format!("involves:{}", u));

    let query = format!("repo:{owner}/{repo} is:issue {user_str} updated:>{n_days_ago}");
    let encoded_query = urlencoding::encode(&query);
    let token_str = match token {
        None => String::new(),
        Some(t) => format!("&token={}", t.as_str()),
    };

    let octocrab = get_octo(&GithubLogin::Default);

    let mut out = Vec::new();

    for _n in 1..3 {
        let url_str = format!(
            "search/issues?q={}&sort=updated&order=desc&per_page=100&page={}{}",
            encoded_query,
            _n,
            token_str
        );
        // let url_str = format!(
        //     "https://api.github.com/search/issues?q={}&sort=updated&order=desc&per_page=100{token_str}",
        //     encoded_query
        // );

        match octocrab.get::<Page<Issue>, _, ()>(&url_str, None::<&()>).await {
            Err(e) => {
                log::error!("Error getting paginated issues: {:?}", e);
                continue;
            }
            Ok(issue_page) => {
                out.extend(issue_page.items.clone().into_iter());
                if &issue_page.items.len() < &100 {
                    break;
                }
            }
        }
    }
    let count = out.len();
    Some((count, out))
}

pub async fn get_commits_in_range_search(
    owner: &str,
    repo: &str,
    user_name: Option<String>,
    range: u16,
    token: Option<String>
) -> Option<(usize, Vec<GitMemory>)> {
    #[derive(Debug, Deserialize)]
    struct Page<T> {
        pub items: Vec<T>,
        pub total_count: Option<u64>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    struct User {
        login: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct GithubCommit {
        sha: String,
        html_url: String,
        author: Option<User>, // made nullable
        commit: CommitDetails,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct CommitDetails {
        message: String,
        // committer: CommitUserDetails,
    }
    let token_str = match &token {
        None => String::from(""),
        Some(t) => format!("&token={}", t.as_str()),
    };
    let author_str = match &user_name {
        None => String::from(""),
        Some(t) => format!("%20author:{}", t.as_str()),
    };
    let now = Utc::now();
    let n_days_ago = (now - Duration::days(range as i64)).date_naive();

    let query = format!("repo:{}/{}{}%20committer-date:>{}", owner, repo, author_str, n_days_ago);
    // let encoded_query = urlencoding::encode(&query);
    let mut git_memory_vec = vec![];
    let octocrab = get_octo(&GithubLogin::Default);

    for _n in 1..3 {
        let url_str = format!(
            "search/commits?q={}&sort=committer-date&order=desc&per_page=100&page={}{}",
            query,
            _n,
            token_str
        );
        // let url_str = format!(
        //     "https://api.github.com/search/commits?q={}&sort=author-date&order=desc&per_page=100{token_str}",
        //     encoded_query
        // );

        match octocrab.get::<Page<GithubCommit>, _, ()>(&url_str, None::<&()>).await {
            Err(e) => {
                log::error!("Error parsing commits: {:?}", e);
            }
            Ok(commits_page) => {
                for commit in &commits_page.items {
                    if let Some(author) = &commit.author {
                        // log::info!("commit author: {:?}", author.clone());
                        git_memory_vec.push(GitMemory {
                            memory_type: MemoryType::Commit,
                            name: author.login.clone(),
                            tag_line: commit.commit.message.clone(),
                            source_url: commit.html_url.clone(),
                            payload: String::from(""),
                        });
                    }
                }
                if &commits_page.items.len() < &100 {
                    break;
                }
            }
        }
    }
    let count = git_memory_vec.len();

    Some((count, git_memory_vec))
}
