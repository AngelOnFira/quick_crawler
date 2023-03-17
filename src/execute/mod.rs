// use futures::stream::{self, StreamExt};
use std::mem::replace;
use async_std::channel::Sender;
use scraper::{Selector};

use std::sync::{Arc};

use crate::{DynRequestHandler, RequestHandlerConfig};
use crate::limiter::Limiter;
use url::Url;
use std::future::Future;

use std::pin::Pin;


use futures::future::{join};








use async_std::{task};
extern crate scraper;
use scraper::{Html};


use crate::scrape::{StartUrl, ResponseLogic::{self, Parallel}, ElementUrlExtractor, ElementDataExtractor,  Ops::{self, *}};
use crate::{DataFromScraperValue, QuickCrawlerError::{self, *}};


async fn limit_url_via<S: Copy +  Into<String>>(limiter: &Option<Arc<Limiter>>, url: S) -> Result<(), QuickCrawlerError> {

    if limiter.is_some() {
        let base = Url::parse(&url.into()).map_err(|_| QuickCrawlerError::ParseDomainErr)?;
        // println!("unwrapping {:?}", url.into().clone());
        let domain = match base.host_str() {
            Some(d) =>d,
            None => return Err(QuickCrawlerError::ParseDomainErr)
        };
        // println!("host_str {:?}", domain);
        // println!("unwrapped {:?}", domain.clone());
        limiter.as_ref().unwrap().limit(domain).await;
    };
    Ok(())
}



pub async fn execute_deep_scrape<'a>(start_url: &StartUrl, data_sender: Sender<DataFromScraperValue>, limiter: Option<Arc<Limiter>>, request_handler: Arc<DynRequestHandler>)-> Result<(), QuickCrawlerError>
{
    let url = match &start_url.url {
        Some(url)=>url,
        None=>{
            return Err(NoUrlInStartUrlErr)
        }
    };

    let req = match &start_url.method {
        // Some(m) if m == "GET" =>"STUB URL REQ GET".to_string(),
        // Some(m) if m == "POST" =>"STUB URL REQ POST".to_string(),
        // // FOR LIVE RESULTS
        Some(m) if m == "GET" =>surf::get(url),
        Some(m) if m == "POST" =>surf::post(url),
        Some(m)=>{
            return Err(InvalidStartUrlMethodErr(m.to_string()))
        }
        None=>{
            return Err(NoStartUrlMethodErr)
        }
    };


    limit_url_via(&limiter, url).await?;
    
    // // FOR LIVE RESULTS

    let html_str = request_handler.call(RequestHandlerConfig{url: url.to_string()}).await.map_err(|_| QuickCrawlerError::RequestErr)?;


    let response_logic = match &start_url.response_logic {
        Some(response_logic)=>response_logic.clone(),
        None=>{
            return Err(NoResponseLogicErr)
        }
    };

    handle_response_logic(&response_logic, url.clone(), html_str, data_sender, limiter, request_handler).await?;

    Ok(())
}




async fn handle_response_logic<'a>(response_logic: &'a ResponseLogic, original_url: String, html_str: String, data_sender: Sender<DataFromScraperValue>, limiter: Option<Arc<Limiter>>, request_handler: Arc<DynRequestHandler>) -> Result<(), QuickCrawlerError>

{
    match response_logic {
        Parallel(par_items) => {
            use futures::stream::{self, StreamExt, TryStreamExt};
            // loop over each Item in array
            let res: Result<(), QuickCrawlerError> = Box::pin(stream::iter(par_items).map(|item| (item, original_url.clone(), data_sender.clone(), html_str.clone(), limiter.clone(), request_handler.clone())).map(Ok).try_for_each_concurrent(
                /* limit */ 4,
                |(scrape, original_url, sender, html_str, limiter, request_handler)| async move {
                    handle_scrape(&scrape.executables, original_url, html_str, sender, limiter, request_handler).await
                }
            )).await;
            res
        }
        _ => {
            return Err(UnknownResponseLogicErr)
        }
    }
}

// fn get_domain_from_url(url: &str)-> Result<String, QuickCrawlerError>{
//     let base = Url::parse(url.into()).map_err(|_| QuickCrawlerError::ParseDomainErr)?;
//     let domain = match base.host_str() {
//         Some(d) => format!("{}://{}", base.scheme(), d).to_string(),
//         None => return Err(QuickCrawlerError::ParseDomainErr)
//     };

//     let domain = match base.port() {
//         Some(p) => format!("{}:{}", domain, p).to_string(),
//         None => domain
//     };

//     Ok(domain.to_string())
// }

fn construct_full_url(original: &str, href: &str)-> Result<String, QuickCrawlerError>{
    // println!("domain: {:?} - href: {:?}", domain, href);
    let res = Url::parse(href.into());
    // println!("base {:?}", base);
    let full_url = match res {
        Ok(d) => href.to_string(),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let base = Url::parse(original).map_err(|_| QuickCrawlerError::ParseDomainErr)?;
            base.join(href).map_err(|_| QuickCrawlerError::ParseDomainErr)?.to_string()
        },
        _ => return Err(QuickCrawlerError::ParseDomainErr)
    };
    // println!("full_url: {:?}", full_url);
    Ok(full_url.clone())
}


#[derive(Debug, Clone)]
struct HtmlContainer {
    original_url: String,
    html_str: String,
    url_node_strs: Vec<String>,
    data_node_strs: Vec<String>,
    node_urls: Vec<String>,
    data_items: Vec<String>,
}


impl HtmlContainer{
    fn new(original_url: String, html_str: String)-> HtmlContainer {
        HtmlContainer{
            original_url,
            html_str,
            url_node_strs: vec![],
            data_node_strs: vec![],
            node_urls: vec![],
            data_items: vec![],
            // next_: vec![],
        }
    }

    fn get_original_url(&self)-> String {
        self.original_url.to_string()
    }
}

use futures::future::{BoxFuture, FutureExt};




fn find_node_strs(pred: &Selector, html_str: &str) -> Vec<String> {
    let mut node_strs = Vec::new();
    // let node_strs = replace(&mut container.node_strs, vec![]);
    Html::parse_fragment(html_str).select(pred).for_each(|node| {
        node_strs.push(node.html().replace('\n', "").trim().to_owned());
    });
    return node_strs
}

fn find_urls(ex: &ElementUrlExtractor, node_strs: &Vec<String>) -> Vec<String> {
    let mut urls = Vec::new();
    // let node_strs = replace(&mut container.node_strs, vec![]);
    node_strs.iter().for_each(|node| {
        let node_el = Html::parse_fragment(&node);

        match ex {
            ElementUrlExtractor::Attr(target_attr) => {
                node_el.root_element().children().for_each(|child| {
                    child
                        .value()
                        .as_element()
                        .and_then(|el| el.attr(target_attr))
                        .map(|url| {
                            // println!("url {:?}", url);
                            urls.push(url.to_owned());
                        });
                })
            }
        };
    });

    return urls
}

fn find_data(ex: &ElementDataExtractor, node_strs: &Vec<String>) -> Vec<String> {
    let mut urls = Vec::new();
    // let node_strs = replace(&mut container.node_strs, vec![]);
    node_strs.iter().for_each(|node| {
        let node_el = Html::parse_fragment(&node);

        match ex {
            ElementDataExtractor::Text => {
                // let element_value = Html::parse_fragment(&node).root_element().value();
                urls.extend(node_el.root_element().text().map(|item| item.trim().to_string()).collect::<Vec<String>>());
            }
        };
    });

    return urls
}



fn handle_scrape<'a>(executables: &'a Vec<Box<Ops>>, original_url: String, html_str: String, data_sender: Sender<DataFromScraperValue>, limiter: Option<Arc<Limiter>>, request_handler: Arc<DynRequestHandler>)-> BoxFuture<'a, Result<(), QuickCrawlerError>>
{
    Box::pin(async move {

        let mut container = HtmlContainer::new(original_url.clone(), html_str.clone());

        for executable in executables.iter() {
            // println!("executable {:?}", i);
            match &**executable {
                UrlSelector(selector_str)=>{
                    let node_strs = find_node_strs(&selector_str, &container.html_str);
                    replace(&mut container.url_node_strs, node_strs);
                }
                DataSelector(selector_str)=>{
                    // println!("Pred!");
                    let node_strs = find_node_strs(&selector_str, &container.html_str);
                    replace(&mut container.data_node_strs, node_strs);
                }
                UrlExtractor(ex)=>{
                    let urls = find_urls(ex, &container.url_node_strs);
                    replace(&mut container.node_urls, urls);
                }
                DataExtractor(ex)=>{
                    let data_items = find_data(ex, &container.data_node_strs);
                    replace(&mut container.data_items, data_items);
                }
                Ops::ResponseLogic(response_logic)=>{

                    // println!("ResponseLogic!");

                    use futures::stream::{self, StreamExt, TryStreamExt};
                    
                    // let hrefs = container.node_urls;
                    

                    // Can't figure out how to remove this block on because 
                    // of Scraper crate dependency that uses Cells :(
                    // println!("{:?}", container.node_urls);
                    // let (sender, receiver) = channel::<DataFromScraperValue>(5);
                    let original_url = container.get_original_url().clone();
                    let res: Result<(), QuickCrawlerError> = Box::pin(stream::iter(&container.node_urls).map(|href| (original_url.clone(), href.clone(), data_sender.clone(), response_logic.clone(), limiter.clone(), request_handler.clone())).map(Ok).try_for_each_concurrent(
                        /* limit */ 5,
                        |(original_url, href, data_sender, response_logic, limiter, request_handler)| async move {
                            // println!("here {:?}", href);
                            let full_url = construct_full_url(&original_url, &href)?;
                            limit_url_via(&limiter, &full_url).await?;
                            
                            // // FOR LIVE RESULTS
                            let html_str = request_handler.call(RequestHandlerConfig{url: full_url.to_string()}).await.map_err(|_| QuickCrawlerError::RequestErr)?;
                            
                            // let html_str = surf::get(&full_url).recv_string().await.map_err(|_| QuickCrawlerError::RequestErr)?;
                            // let html_str = format!("<div class='ingredients-prep'><div class='ingredient'>{} test ingredent</div><div class='ingredient'>{} test ingredent</div><div class='prep-steps'><li>step: {}</li></div></div>", i, i, i);
                            handle_response_logic(&response_logic, full_url, html_str, data_sender, limiter, request_handler).await

                            // async_std::task::yield_now().await;
                            // Ok(())
                        }
                    )).await;

                    res?;
                }
                Store(f)=>{
                    let res = container.data_items.iter().map(|x| x.to_string()).collect::<Vec<String>>();
                    f.call(res).await;
                }
            }

        }
        Ok(())
    })
}







