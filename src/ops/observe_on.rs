use crate::prelude::*;
use crate::scheduler::SharedScheduler;
use observable::observable_proxy_impl;
use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct ObserveOnOp<S, SD> {
  pub(crate) source: S,
  pub(crate) scheduler: SD,
}

observable_proxy_impl!(ObserveOnOp, S, SD);

impl<S, SD> LocalObservable<'static> for ObserveOnOp<S, SD>
where
  S: LocalObservable<'static>,
  S::Item: Clone + 'static,
  S::Err: Clone + 'static,
  SD: LocalScheduler + 'static,
{
  type Unsub = S::Unsub;
  fn actual_subscribe<O: Observer<Self::Item, Self::Err> + 'static>(
    self,
    subscriber: Subscriber<O, LocalSubscription>,
  ) -> Self::Unsub {
    let Subscriber {
      observer,
      subscription,
    } = subscriber;
    let observer = LocalObserver {
      observer: Rc::new(RefCell::new(observer)),
      scheduler: self.scheduler,
      subscription: subscription.clone(),
    };

    self.source.actual_subscribe(Subscriber {
      observer: observer,
      subscription,
    })
  }
}

impl<S, SD> SharedObservable for ObserveOnOp<S, SD>
where
  S: SharedObservable,
  S::Item: Clone + Send + 'static,
  S::Err: Clone + Send + 'static,
  SD: SharedScheduler + Send + Sync + 'static,
{
  type Unsub = S::Unsub;
  fn actual_subscribe<
    O: Observer<Self::Item, Self::Err> + Sync + Send + 'static,
  >(
    self,
    subscriber: Subscriber<O, SharedSubscription>,
  ) -> Self::Unsub {
    let Subscriber {
      observer,
      subscription,
    } = subscriber;
    let subscriber = SharedObserver {
      observer: Arc::new(Mutex::new(observer)),
      subscription: subscription.clone(),
      scheduler: self.scheduler,
    };
    self.source.actual_subscribe(Subscriber {
      observer: subscriber,
      subscription,
    })
  }
}

struct LocalObserver<O, SD: LocalScheduler> {
  observer: Rc<RefCell<O>>,
  scheduler: SD,
  subscription: LocalSubscription,
}

struct SharedObserver<O, SD: SharedScheduler> {
  observer: Arc<Mutex<O>>,
  subscription: SharedSubscription,
  scheduler: SD,
}

#[doc(hidden)]
macro impl_observer($item: ident, $err: ident) {
  fn next(&mut self, value: $item) {
    self.observer_schedule(move |mut observer, v| observer.next(v), value)
  }
  fn error(&mut self, err: $err) {
    self.observer_schedule(|mut observer, v| observer.error(v), err)
  }
  fn complete(&mut self) {
    self.observer_schedule(|mut observer, _| observer.complete(), ())
  }
}

impl<O, SD> SharedObserver<O, SD>
where
  SD: SharedScheduler,
{
  fn observer_schedule<S, Task>(&mut self, task: Task, state: S)
  where
    S: Send + 'static,
    O: Send + 'static,
    Task: FnOnce(Arc<Mutex<O>>, S) + Send + 'static,
  {
    let subscription = self.scheduler.schedule(
      |_, (observer, state)| task(observer, state),
      None,
      (self.observer.clone(), state),
    );

    self.subscription.add(subscription);
  }
}

impl<Item, Err, O, SD> Observer<Item, Err> for SharedObserver<O, SD>
where
  Item: Clone + Send + 'static,
  Err: Clone + Send + 'static,
  O: Observer<Item, Err> + Send + 'static,
  SD: SharedScheduler,
{
  impl_observer!(Item, Err);
}

impl<O: 'static, SD: LocalScheduler + 'static> LocalObserver<O, SD> {
  fn observer_schedule<S, Task>(&mut self, task: Task, state: S)
  where
    S: 'static,
    Task: FnOnce(Rc<RefCell<O>>, S) + 'static,
  {
    let subscription = self.scheduler.schedule(
      |_, (observer, state)| task(observer, state),
      None,
      (self.observer.clone(), state),
    );

    self.subscription.add(subscription);
  }
}
impl<Item, Err, O, SD> Observer<Item, Err> for LocalObserver<O, SD>
where
  Item: Clone + 'static,
  Err: Clone + 'static,
  O: Observer<Item, Err> + 'static,
  SD: LocalScheduler + 'static,
{
  impl_observer!(Item, Err);
}

#[cfg(test)]
mod test {
  use crate::prelude::*;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::{Arc, Mutex};
  use std::thread;
  use std::time::Duration;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn smoke() {
    let v = Rc::new(RefCell::new(0));
    let v_c = v.clone();
    let mut local = LocalPool::new();
    observable::of(1)
      .observe_on(local.spawner())
      .subscribe(move |i| *v_c.borrow_mut() = i);
    local.run();

    assert_eq!(*v.borrow(), 1);
  }

  #[test]
  fn switch_thread() {
    let id = thread::spawn(move || {}).thread().id();
    let emit_thread = Arc::new(Mutex::new(id));
    let observe_thread = Arc::new(Mutex::new(vec![]));
    let oc = observe_thread.clone();

    let pool = ThreadPool::builder().pool_size(100).create().unwrap();

    observable::create(|mut s| {
      s.next(&1);
      s.next(&1);
      *emit_thread.lock().unwrap() = thread::current().id();
    })
    .observe_on(pool.clone())
    .to_shared()
    .subscribe(move |_v| {
      observe_thread.lock().unwrap().push(thread::current().id());
    });
    std::thread::sleep(Duration::from_millis(1));

    let current_id = thread::current().id();
    assert_eq!(*emit_thread.lock().unwrap(), current_id);
    let ot = oc.lock().unwrap();
    let ot1 = ot[0];
    let ot2 = ot[1];
    assert_ne!(ot1, ot2);
    assert_ne!(current_id, ot2);
    assert_ne!(current_id, ot1);
  }

  #[test]
  fn pool_unsubscribe() {
    let scheduler = ThreadPool::new().unwrap();
    let emitted = Arc::new(Mutex::new(vec![]));
    let c_emitted = emitted.clone();
    observable::from_iter(0..10)
      .to_shared()
      .observe_on(scheduler.clone())
      .delay(Duration::from_millis(10), scheduler)
      .to_shared()
      .subscribe(move |v| {
        emitted.lock().unwrap().push(v);
      })
      .unsubscribe();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(c_emitted.lock().unwrap().len(), 0);
  }
}
