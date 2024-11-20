use yew::AttrValue;
use yew_router::Routable;

#[derive(Clone, PartialEq, Routable)]
pub enum Route {
    #[at("/:code/picture")]
    Board { code: AttrValue },
}