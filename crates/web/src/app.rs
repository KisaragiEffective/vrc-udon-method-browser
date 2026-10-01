use leptos::prelude::*;
use vrc_udon_methods_core::{MethodRecord, search};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{HtmlInputElement, HtmlSelectElement};

use crate::files::{download_bytes, looks_like_zip};
use crate::remote::{RemoteSdk, load_remote_sdks, sdk_proxy_base};

#[component]
pub fn App() -> impl IntoView {
    let (records, set_records) = signal(Vec::<MethodRecord>::new());
    let (query, set_query) = signal(String::new());
    let (status, set_status) = signal(String::from(
        "Select a remote World SDK version or upload a local SDK zip/DLL set.",
    ));
    let (remote_sdks, set_remote_sdks) = signal(Vec::<RemoteSdk>::new());
    let (selected_remote, set_selected_remote) = signal(String::new());
    let filtered = move || search(&records.get(), &query.get());

    Effect::new(move |_| {
        spawn_local(async move {
            set_status.set("Loading official World SDK releases...".to_owned());
            match load_remote_sdks().await {
                Ok(sdks) => {
                    let first = sdks.first().map(|sdk| sdk.id.clone()).unwrap_or_default();
                    set_selected_remote.set(first);
                    set_status.set(format!("Loaded {} remote source option(s).", sdks.len()));
                    set_remote_sdks.set(sdks);
                }
                Err(error) => set_status.set(format!("Failed to load remote releases: {error}")),
            }
        });
    });

    let load_sample = move |_| {
        let symbols = vec![
            "SystemBoolean.__TryParse__SystemString_SystemBooleanRef__SystemBoolean".to_owned(),
            "SystemArray.__Clear__SystemArray_SystemInt32_SystemInt32__SystemVoid".to_owned(),
        ];
        set_records.set(vrc_udon_methods_core::parse_symbols("sample", symbols));
        set_status.set("Loaded sample symbols.".to_owned());
    };

    let load_remote = move |_| {
        let tag = selected_remote.get();
        let sdk = remote_sdks.get().into_iter().find(|sdk| sdk.id == tag);
        let Some(sdk) = sdk else {
            set_status.set("No remote SDK version selected.".to_owned());
            return;
        };

        spawn_local(async move {
            set_status.set(format!("Downloading World SDK {}...", sdk.version));
            let url = format!("{}{}/{}.zip", sdk_proxy_base(), "/sdk", sdk.version);
            match download_bytes(&url).await {
                Ok(bytes) => {
                    match vrc_udon_methods_extractor::extract_zip_bytes(&sdk.version, &bytes) {
                        Ok(next_records) => {
                            let count = next_records.len();
                            set_records.set(next_records);
                            set_status.set(format!(
                                "Extracted {count} symbols from release {}.",
                                sdk.version
                            ));
                        }
                        Err(error) => set_status.set(error.to_string()),
                    }
                }
                Err(error) => {
                    set_status.set(format!("Failed to download release asset: {error}"));
                }
            }
        });
    };

    let on_files = move |ev| {
        let input = event_target::<HtmlInputElement>(&ev);
        let Some(files) = input.files() else {
            set_status.set("No files selected.".to_owned());
            return;
        };
        set_status.set(format!("Reading {} file(s)...", files.length()));
        spawn_local(async move {
            let mut buffers = Vec::new();
            for index in 0..files.length() {
                let Some(file) = files.item(index) else {
                    continue;
                };

                match JsFuture::from(file.array_buffer()).await {
                    Ok(buffer) => {
                        let array = js_sys::Uint8Array::new(&buffer);
                        let mut bytes = vec![0; array.length() as usize];
                        array.copy_to(&mut bytes);
                        buffers.push(bytes);
                    }
                    Err(_) => {
                        set_status.set("Failed to read one of the selected files.".to_owned());
                        return;
                    }
                }
            }

            let extraction = if buffers.len() == 1 && looks_like_zip(&buffers[0]) {
                vrc_udon_methods_extractor::extract_zip_bytes("local-upload", &buffers[0])
            } else {
                vrc_udon_methods_extractor::extract_bytes("local-upload", &buffers)
            };

            match extraction {
                Ok(next_records) => {
                    let count = next_records.len();
                    set_records.set(next_records);
                    set_status.set(format!("Extracted {count} mangled symbols."));
                }
                Err(error) => set_status.set(error.to_string()),
            }
        });
    };

    view! {
        <main class="app-shell">
            <section class="toolbar">
                <div>
                    <h1>"VRC Udon Methods"</h1>
                    <p>"SDK DLLs and generated symbol tables are not bundled."</p>
                </div>
                <button on:click=load_sample>"Load sample"</button>
            </section>
            <section class="loader">
                <label for="remote-version">"Remote version"</label>
                <select
                    id="remote-version"
                    on:change=move |ev| {
                        let select = event_target::<HtmlSelectElement>(&ev);
                        set_selected_remote.set(select.value());
                    }
                >
                    <For
                        each=move || remote_sdks.get()
                        key=|sdk| sdk.id.clone()
                        children=|sdk| {
                            let value = sdk.id.clone();
                            let label = sdk.label;
                            view! { <option value=value>{label}</option> }
                        }
                    />
                </select>
                <button on:click=load_remote>"Download"</button>
            </section>
            <section class="loader">
                <label for="upload-sdk">"Upload"</label>
                <input
                    id="upload-sdk"
                    type="file"
                    multiple
                    accept=".dll,.zip"
                    on:change=on_files
                />
            </section>
            <p class="status">{move || status.get()}</p>
            <input
                class="search"
                placeholder="Search declaring type, member, or mangled symbol"
                on:input=move |ev| set_query.set(event_target_value(&ev))
            />
            <section class="results">
                <For
                    each=filtered
                    key=|record| record.symbol.clone()
                    children=|record| view! { <MethodRow record=record /> }
                />
            </section>
        </main>
    }
}

#[component]
fn MethodRow(record: MethodRecord) -> impl IntoView {
    let params = record
        .parameters
        .iter()
        .map(|param| param.display_name())
        .collect::<Vec<_>>()
        .join(", ");
    let ret = record
        .return_type
        .as_ref()
        .map(|ty| ty.display_name())
        .unwrap_or_else(|| "void".to_owned());

    view! {
        <article class="method-row">
            <div class="method-title">
                <strong>{record.declaring_type_fqcn.clone().unwrap_or(record.declaring_type.clone())}</strong>
                <span>"."{record.member_name.clone()}</span>
            </div>
            <code>{record.symbol}</code>
            <div class="signature">{format!("({params}) -> {ret}")}</div>
        </article>
    }
}
