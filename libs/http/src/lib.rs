use std::collections::HashMap;

use koto_runtime::{DataMap, RuntimeError, Value, ValueKey, ValueMap, runtime_error};
use ureq::{OrAnyStatus, Response};
use ureq::Error;

pub fn make_module() -> koto_runtime::ValueMap {
    let mut module = koto_runtime::ValueMap::new();
    module.add_fn("id", |vm, args|{
	match vm.get_args(args){
	    [Value::Str(method), Value::Str(url)] =>
		koto_http_request(method, url).map_err(|s|s.into()),
	    _ => runtime_error!("")
	}
    });
    module
}

fn get_response_headers(response: &Response) -> ValueMap {
    ValueMap::with_data(
	response.headers_names().iter()
	    // .map(|name| (ValueKey::from(name.as_str()),
	    .map(|name| (name.as_str().into(),
			 response.header(&name).unwrap().into()))
	    .collect())
}

fn koto_http_request(method: &str, url: &str) -> Result<Value, String>{
    let response = ureq::request(method, url)
        .set("Example-Header", "header value")
        .call().unwrap();

    let mut koto_response = DataMap::new();

    koto_response.insert("headers".into(),
			 Value::Map(get_response_headers(&response)));
    koto_response.insert("http_version".into(), response.http_version().into());
    koto_response.insert("status".into(), response.status().into());
    koto_response.insert("status_text".into(), response.status_text().into());
    koto_response.insert("url".into(), response.get_url().into());
    koto_response.insert("body".into(), response.into_string().unwrap().into());
    Ok(Value::Map(ValueMap::with_data(koto_response)))
}


#[cfg(test)]
mod test {
    use crate::make_module;

    #[test]
    fn test_make_module(){
	assert_eq!(koto_runtime::ValueMap::new().len(), make_module().len());
    }

}
