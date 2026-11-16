//! The system clipboard. A refused read returns no items, or items without data, rather than
//! failing; such contents are reported as unreadable and never put back as an empty clipboard.
//! Every call runs in its own autorelease pool, so threads without one do not leak.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dictation_types::{AppError, AppResult};
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardAccessBehavior, NSPasteboardItem, NSPasteboardType,
    NSPasteboardTypeOwner, NSPasteboardTypeString, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSString};

use crate::clipboard_contents::ReadItem;
use crate::insertion::{Clipboard, ClipboardContents, ClipboardSnapshot, TextOffer};

/// The type that marks clipboard contents as about to be replaced, so clipboard history tools
/// leave them out.
const TRANSIENT_TYPE: &str = "org.nspasteboard.TransientType";

struct OwnerIvars {
    text: String,
    offer: Arc<TextOffer>,
}

define_class!(
    /// Hands promised text to whoever asks for it and tells the offer about it.
    #[unsafe(super(NSObject))]
    #[name = "PlatformMacosTextPromise"]
    #[ivars = OwnerIvars]
    struct PromiseOwner;

    unsafe impl NSObjectProtocol for PromiseOwner {}

    unsafe impl NSPasteboardTypeOwner for PromiseOwner {
        #[unsafe(method(pasteboard:provideDataForType:))]
        fn provide(&self, sender: &NSPasteboard, kind: &NSPasteboardType) {
            // SAFETY: a constant string the system framework defines.
            let string_type = unsafe { NSPasteboardTypeString };
            if kind.isEqualToString(string_type) {
                sender.setString_forType(&NSString::from_str(&self.ivars().text), kind);
                self.ivars().offer.note_read();
            } else {
                // The transient mark carries no data, and reading it is no paste.
                sender.setData_forType(Some(&NSData::new()), kind);
            }
        }

        #[unsafe(method(pasteboardChangedOwner:))]
        fn changed_owner(&self, _sender: &NSPasteboard) {
            self.ivars().offer.note_replaced();
        }
    }
);

// SAFETY: the ivars never change after creation and are themselves safe to share.
unsafe impl Send for PromiseOwner {}
// SAFETY: as above.
unsafe impl Sync for PromiseOwner {}

impl PromiseOwner {
    fn new(text: &str, offer: &Arc<TextOffer>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(OwnerIvars {
            text: text.to_owned(),
            offer: Arc::clone(offer),
        });
        // SAFETY: `init` of the superclass, called once on a fresh allocation.
        unsafe { msg_send![super(this), init] }
    }
}

/// The owner of the text promised last, kept alive for as long as the promise may be asked
/// for, since the clipboard does not keep it alive itself.
#[derive(Default)]
struct Promise(Mutex<Option<Retained<PromiseOwner>>>);

impl Promise {
    fn lock(&self) -> MutexGuard<'_, Option<Retained<PromiseOwner>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl fmt::Debug for Promise {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Promise")
            .field("pending", &self.lock().is_some())
            .finish()
    }
}

/// A system clipboard: the general one that copy and paste use, or a separate one by name.
#[derive(Debug, Default, Clone)]
pub struct SystemClipboard {
    name: Option<String>,
    promise: Arc<Promise>,
}

impl SystemClipboard {
    pub fn general() -> Self {
        Self::default()
    }

    /// A separate clipboard of its own name, which nothing else uses unless it asks for the
    /// same name. Tests use one so they leave the user's clipboard alone.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            promise: Arc::default(),
        }
    }

    /// Lets the system drop a named clipboard. Does nothing for the general one.
    pub fn release(&self) {
        if self.name.is_some() {
            autoreleasepool(|_| {
                let pasteboard = self.pasteboard();
                // SAFETY: `releaseGlobally` takes no arguments and returns nothing.
                unsafe {
                    let () = msg_send![&*pasteboard, releaseGlobally];
                }
            });
        }
    }

    fn pasteboard(&self) -> Retained<NSPasteboard> {
        match &self.name {
            None => NSPasteboard::generalPasteboard(),
            Some(name) => NSPasteboard::pasteboardWithName(&NSString::from_str(name)),
        }
    }

    /// Called inside an autorelease pool.
    fn write_items(&self, items: Vec<Retained<NSPasteboardItem>>) -> AppResult<i64> {
        let pasteboard = self.pasteboard();
        pasteboard.clearContents();
        if !items.is_empty() {
            let objects: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = items
                .into_iter()
                .map(ProtocolObject::from_retained)
                .collect();
            if !pasteboard.writeObjects(&NSArray::from_retained_slice(&objects)) {
                return Err(AppError::injection(
                    "the clipboard did not take the new contents",
                ));
            }
        }
        // Whatever was promised is gone from the clipboard now.
        self.promise.lock().take();
        Ok(count(&pasteboard))
    }
}

/// Whether the system says this application may never read the clipboard. Older systems lack
/// the setting and always allow reading.
fn reads_denied(pasteboard: &NSPasteboard) -> bool {
    pasteboard.respondsToSelector(sel!(accessBehavior))
        && pasteboard.accessBehavior() == NSPasteboardAccessBehavior::AlwaysDeny
}

fn count(pasteboard: &NSPasteboard) -> i64 {
    i64::try_from(pasteboard.changeCount()).unwrap_or(i64::MAX)
}

impl Clipboard for SystemClipboard {
    fn contents(&self) -> AppResult<ClipboardSnapshot> {
        autoreleasepool(|_| {
            let pasteboard = self.pasteboard();
            if reads_denied(&pasteboard) {
                return Ok(ClipboardSnapshot::Unreadable);
            }
            let Some(items) = pasteboard.pasteboardItems() else {
                return Ok(ClipboardSnapshot::Unreadable);
            };
            let read = items.to_vec().into_iter().map(|item| -> ReadItem {
                item.types()
                    .to_vec()
                    .into_iter()
                    .map(|kind| {
                        let kind: &NSPasteboardType = &kind;
                        let data = item.dataForType(kind).map(|data| data.to_vec());
                        (kind.to_string(), data)
                    })
                    .collect()
            });
            Ok(ClipboardSnapshot::from_read(read))
        })
    }

    fn write_text(&self, text: &str, transient: bool) -> AppResult<i64> {
        autoreleasepool(|_| {
            let item = NSPasteboardItem::new();
            // SAFETY: a constant string the system framework defines.
            let string_type = unsafe { NSPasteboardTypeString };
            if !item.setString_forType(&NSString::from_str(text), string_type) {
                return Err(AppError::injection("the clipboard did not take the text"));
            }
            if transient {
                item.setData_forType(&NSData::new(), &NSString::from_str(TRANSIENT_TYPE));
            }
            self.write_items(vec![item])
        })
    }

    fn offer_text(&self, text: &str, offer: &Arc<TextOffer>) -> AppResult<i64> {
        autoreleasepool(|_| {
            let pasteboard = self.pasteboard();
            let owner = PromiseOwner::new(text, offer);
            // SAFETY: a constant string the system framework defines.
            let string_type = unsafe { NSPasteboardTypeString };
            let transient = NSString::from_str(TRANSIENT_TYPE);
            let types = NSArray::from_slice(&[string_type, &*transient]);
            // SAFETY: the owner answers both types it is declared for, and `promise` keeps it
            // alive until the clipboard is written again.
            let declared = unsafe { pasteboard.declareTypes_owner(&types, Some(&owner)) };
            *self.promise.lock() = Some(owner);
            Ok(i64::try_from(declared).unwrap_or(i64::MAX))
        })
    }

    fn restore(&self, contents: &ClipboardContents) -> AppResult<()> {
        autoreleasepool(|_| {
            let items = contents
                .items
                .iter()
                .map(|types| {
                    let item = NSPasteboardItem::new();
                    for (kind, data) in types {
                        item.setData_forType(&NSData::with_bytes(data), &NSString::from_str(kind));
                    }
                    item
                })
                .collect();
            self.write_items(items).map(|_| ())
        })
    }

    fn change_count(&self) -> AppResult<i64> {
        autoreleasepool(|_| Ok(count(&self.pasteboard())))
    }
}
