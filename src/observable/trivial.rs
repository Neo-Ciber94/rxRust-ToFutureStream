use crate::prelude::*;

/// Creates an observable that emits no items, just terminates with an error.
///
/// # Arguments
///
/// * `e` - An error to emit and terminate with
pub fn throw<Err>(e: Err) -> ThrowObservable<Err> {
  ThrowObservable(e)
}

#[derive(Clone)]
pub struct ThrowObservable<Err>(Err);

impl<Err, O> Observable<(), Err, O> for ThrowObservable<Err>
where
  O: Observer<(), Err>,
{
  type Unsub = ();

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    observer.error(self.0);
  }
}

impl<Err> ObservableExt<(), Err> for ThrowObservable<Err> {}

/// Creates an observable that produces no values.
///
/// Completes immediately. Never emits an error.
///
/// # Examples
/// ```
/// use rxrust::prelude::*;
///
/// observable::empty()
///   .subscribe(|v: &i32| {println!("{},", v)});
///
/// // Result: no thing printed
/// ```
#[inline]
pub fn empty<Item>() -> EmptyObservable<Item> {
  EmptyObservable(TypeHint::new())
}

#[derive(Clone)]
pub struct EmptyObservable<Item>(TypeHint<Item>);

impl<Item, O> Observable<Item, (), O> for EmptyObservable<Item>
where
  O: Observer<Item, ()>,
{
  type Unsub = ();

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    observer.complete();
  }
}

impl<Item> ObservableExt<Item, ()> for EmptyObservable<Item> {}
/// Creates an observable that never emits anything.
///
/// Neither emits a value, nor completes, nor emits an error.
#[inline]
pub fn never() -> NeverObservable {
  NeverObservable
}

#[derive(Clone)]
pub struct NeverObservable;

impl<O> Observable<(), (), O> for NeverObservable
where
  O: Observer<(), ()>,
{
  type Unsub = ();

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    observer.complete();
  }
}

impl ObservableExt<(), ()> for NeverObservable {}
#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn throw() {
    let mut value_emitted = false;
    let mut completed = false;
    let mut error_emitted = String::new();
    observable::throw(String::from("error"))
      .on_error(|e| error_emitted = e)
      .on_complete(|| completed = true)
      .subscribe(
        // helping with type inference
        |_| value_emitted = true,
      );
    assert!(!value_emitted);
    assert!(!completed);
    assert_eq!(error_emitted, "error");
  }

  #[test]
  fn empty() {
    let mut hits = 0;
    let mut completed = false;
    observable::empty()
      .on_complete(|| completed = true)
      .subscribe(|()| hits += 1);

    assert_eq!(hits, 0);
    assert!(completed);
  }
}
