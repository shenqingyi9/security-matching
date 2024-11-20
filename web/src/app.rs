use yew::prelude::*;
use yew_router::{BrowserRouter, Switch};
use crate::security::Security;

use crate::route::Route;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} /> // <- must be child of <BrowserRouter>
        </BrowserRouter>
    }
}

fn switch(routes: Route) -> Html {
    let url = AttrValue::Static("/api");
    match routes {
        Route::Board { code } => html! { <Security api={format!("{}/watch/{}", url, code)} /> },
    }
}