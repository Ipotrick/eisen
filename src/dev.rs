use std::{pin::Pin, sync::Arc};

use futures::Future;

use crate::{app::{SharedAppData, FixedData, User}, entity::RectRenderable, sync::block_on};

use crate::entity::Transform;

pub struct MyUser {

}

impl User for MyUser {
    fn init(self: Arc<Self>, app: Arc<SharedAppData>) {
        let init = async{
            get_entities_mut!(manager: app.ecm => entities);
            get_components_mut!(manager: app.ecm; components: Transform, RectRenderable => transforms, rectrenders);

            for _ in 0..100 {
                let new_ent = entities.create();
                entities.add(transforms, Transform::default(), new_ent);
                entities.add(rectrenders, RectRenderable::default(), new_ent);
            }
        };

        block_on(init);
    }

    fn cleanup(self: Arc<Self>, _: Arc<SharedAppData>)  { 
        println!("user cleanup!");
    }

    fn vary_tick(self: Arc<Self>, app: Arc<SharedAppData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>{ 
        let app = app.clone();
        Box::pin(async move {
            //println!("delta time vary: {}s", app.get_prev_frame_delta_time_secs());
        })
    }
    
    fn fixed_tick(self: Arc<Self>, _: Arc<SharedAppData>, _: Arc<FixedData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>{ 
        Box::pin(async{
            profiling::scope!("fixed_tick_user","");
            spin_sleep::sleep(std::time::Duration::from_millis(100));
        }) 
    }
}