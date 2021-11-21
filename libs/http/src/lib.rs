
use std::io::{Error, ErrorKind};

use koto_runtime::{DataMap, Value, ValueKey, ValueMap, runtime_error};
use ureq::Response;

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

fn get_response_headers(response: &Response) -> Option<ValueMap> {
    Some(ValueMap::with_data(
	response.headers_names().iter()
	    .map(|name|
		 response.header(&name)
		 .map(|header_content|
		      (ValueKey::from(name.as_str()),
		       Value::from(header_content))))
	    .collect::<Option<DataMap>>()?))
}


fn into_koto_response(response: ureq::Response) -> Result<DataMap, Error>{
    let mut koto_response = DataMap::new();

    let headers = get_response_headers(&response)
	.ok_or(Error::new(ErrorKind::Other, "failed to get headers (this should never happen)"))?;
    koto_response.add_map("headers", headers);
    koto_response.add_value("http_version", response.http_version());
    koto_response.add_value("status", response.status());
    koto_response.add_value("status_text", response.status_text());
    koto_response.add_value("url", response.get_url());
    koto_response.add_value("body", response.into_string()?);

    Ok(koto_response)
}
fn koto_http_request(method: &str, url: &str) -> Result<Value, String>{
    let response = ureq::request(method, url)
        .call().or_else(|e|Err(e.to_string()))?;

    into_koto_response(response)
        .map(ValueMap::with_data)
        .map(Value::Map)
        .map_err(|e|e.to_string())
}


#[cfg(test)]
mod test {
    use crate::make_module;

    #[test]
    fn test_make_module(){
	assert_eq!(koto_runtime::ValueMap::new().len(), make_module().len());
    }

}
