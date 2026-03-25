## The reference answer to the response handling for both modes

### CLI: the result is also configurable, like
service_name: my-service

gateway:                                      
    addr: localhost:50051                     
    token: ${XGENT_NODE_TOKEN}

execution:                                    
    type: cli                                  
    command: ["/usr/bin/python3", "run.py", "<payload>"]
    input: arg                                 
    timeout_secs: 30                           
    cwd: /opt/scripts                          
    env:                                       
        MODEL_NAME: gpt-4
  or
type: cli
settings:
  cmd: "echo <payload> | python my_script.py"
  success:
    body: |'
      status: ok
      data: <stdout>
      '
  failed:
    body:
      status: error
      error: <stderr>

  or

  something else with <stdout> and <stderr> placeholder

### HTTP: the result can be configured like
service_name: my-service

gateway:                                      
    addr: localhost:50051                     
    token: ${XGENT_NODE_TOKEN}

type: async-api # sync-api, or cli

execution:                                    
    type: cli                                  
    command: ["/usr/bin/python3", "run.py", "<payload>"]
    input: arg                                 
    timeout_secs: 30                           
    cwd: /opt/scripts                          
    env:                                       
        MODEL_NAME: gpt-4
settings:
    submit:
        url: "http://localhost:8080/api/v1/services/aigc/video-generation/video-synthesis"
        headers: 
        Authorization: "Bearer ${SOMEWHERE_API_KEY}"
        method: POST
        body: <payload>
    poll:
        url: "http://localhost:8080/api/v1/tasks/<submit_response.output.task_id>"
        interval: 1s
        headers: 
        Authorization: "Bearer ${SOMEWHERE_API_KEY}"
        method: GET
        completed_when: 
            value: <submit_response.output.task_status>
            equal: 'SUCCEEDED'  # can be 'in', 'not_in', 'not_equal'
        body: 
            status: <submit_response.output.task_status>
            data: <submit_response.output>
            usage: <submit_response.usage>
      